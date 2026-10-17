use anyhow::{Context, Result};
use jamseq_plugin_api::{
    allocate_c_note_name_list, allocate_c_parameter_list, JamSeqAudioBuffer, JamSeqMidiEvent,
    JamSeqNoteName, JamSeqNoteNameList, JamSeqParamType, JamSeqParameterList,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vst3_host::audio::AudioBuffers;
use vst3_host::host::Vst3Host;
use vst3_host::midi::MidiChannel;
use vst3_host::plugin::Plugin;

const GM_DRUM_MAP: &[(u8, &str)] = &[
    (35, "Acoustic Bass Drum"),
    (36, "Bass Drum 1"),
    (37, "Side Stick"),
    (38, "Acoustic Snare"),
    (39, "Hand Clap"),
    (40, "Electric Snare"),
    (41, "Low Floor Tom"),
    (42, "Closed Hi-Hat"),
    (43, "High Floor Tom"),
    (44, "Pedal Hi-Hat"),
    (45, "Low Tom"),
    (46, "Open Hi-Hat"),
    (47, "Low-Mid Tom"),
    (48, "Hi-Mid Tom"),
    (49, "Crash Cymbal 1"),
    (50, "High Tom"),
    (51, "Ride Cymbal 1"),
    (52, "Chinese Cymbal"),
    (53, "Ride Bell"),
    (54, "Tambourine"),
    (55, "Splash Cymbal"),
    (56, "Cowbell"),
    (57, "Crash Cymbal 2"),
];

#[derive(Clone, Debug)]
pub struct Vst3ParamInternal {
    pub id: u32,
    pub name: String,
    pub group: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub value: f32,
    pub param_type: JamSeqParamType,
}

pub struct Vst3Instance {
    pub path: String,
    pub plugin_name: String,
    plugin: Arc<Mutex<Plugin>>,
    buffers: Arc<Mutex<AudioBuffers>>,
    params: Arc<Mutex<Vec<Vst3ParamInternal>>>,
    param_index: Arc<Mutex<HashMap<String, usize>>>,
    note_names: Arc<Mutex<Vec<JamSeqNoteName>>>,
    sample_rate: f64,
}

impl Vst3Instance {
    pub fn load(path: &str, sample_rate: f64, max_block_size: u32) -> Result<Self> {
        let clean_path = path.strip_prefix("vst3:").unwrap_or(path);
        let block_size = (max_block_size.max(512)) as usize;

        let mut host = Vst3Host::builder()
            .sample_rate(sample_rate)
            .block_size(block_size)
            .with_process_isolation(false)
            .build()
            .context("Failed to build Vst3Host")?;

        let mut plugin = host
            .load_plugin(clean_path)
            .with_context(|| format!("Failed to load VST3 plugin from: {}", clean_path))?;

        let plugin_name = plugin.info().name.clone();

        plugin
            .start_processing()
            .with_context(|| format!("Failed to start processing for VST3 plugin: {}", plugin_name))?;

        let raw_params = plugin.get_parameters().unwrap_or_default();
        let mut params = Vec::new();
        let mut param_index = HashMap::new();

        for (i, p) in raw_params.into_iter().enumerate() {
            let (group, display_name) = if let Some(slash_idx) = p.name.rfind('/') {
                let g = p.name[..slash_idx].trim().to_string();
                let n = p.name[slash_idx + 1..].trim().to_string();
                (
                    if g.is_empty() {
                        "General".to_string()
                    } else {
                        g.replace('/', " > ")
                    },
                    if n.is_empty() { p.name.clone() } else { n },
                )
            } else if !p.unit.is_empty() && (p.name.contains(':') || p.name.contains(" - ")) {
                if let Some(colon_idx) = p.name.find(':') {
                    let g = p.name[..colon_idx].trim().to_string();
                    let n = p.name[colon_idx + 1..].trim().to_string();
                    (g, n)
                } else {
                    ("General".to_string(), p.name.clone())
                }
            } else {
                ("General".to_string(), p.name.clone())
            };

            let group = if group == "General" {
                let n = display_name.to_lowercase();
                if n.contains("cutoff")
                    || n.contains("reso")
                    || n.contains("drive")
                    || n.contains("filter")
                {
                    "Filter".to_string()
                } else if n.contains("attack")
                    || n.contains("decay")
                    || n.contains("sustain")
                    || n.contains("release")
                    || n.contains("env")
                {
                    "Amp Envelope".to_string()
                } else if n.contains("glide")
                    || n.contains("voice")
                    || n.contains("portamento")
                    || n.contains("tuning")
                {
                    "Voice".to_string()
                } else if n.contains("gain")
                    || n.contains("volume")
                    || n.contains("master")
                    || n.contains("pan")
                {
                    "Output".to_string()
                } else {
                    "General".to_string()
                }
            } else {
                group
            };

            let is_bypass = p.is_bypass;
            let step_count = p.step_count;
            let param_type = if is_bypass || step_count == 1 {
                JamSeqParamType::Bool
            } else if step_count > 1 && step_count <= 64 {
                JamSeqParamType::Enum
            } else if step_count > 64 {
                JamSeqParamType::Int
            } else {
                let n = p.name.to_lowercase();
                let is_likely_enum = n.contains("type")
                    || n.contains("mode")
                    || n.contains("model")
                    || n.contains("waveform")
                    || n.contains("algorithm")
                    || n.contains("shape");
                if is_likely_enum {
                    JamSeqParamType::Enum
                } else {
                    JamSeqParamType::Float
                }
            };

            let param = Vst3ParamInternal {
                id: p.id,
                name: display_name,
                group,
                param_type,
                min: p.min as f32,
                max: p.max as f32,
                default: p.default as f32,
                value: p.value as f32,
            };

            param_index.insert(param.name.clone(), i);
            param_index.insert(param.name.to_lowercase(), i);
            param_index.insert(param.name.to_lowercase().replace(' ', "_"), i);
            param_index.insert(param.name.to_lowercase().replace('_', " "), i);
            if p.name != param.name {
                param_index.insert(p.name.clone(), i);
                param_index.insert(p.name.to_lowercase(), i);
            }
            params.push(param);
        }

        // Introspect mapped drum names (GM fallback if drum/percussion kit)
        let mut mapped_notes = Vec::new();
        let lower = plugin_name.to_lowercase();
        if lower.contains("drum")
            || lower.contains("kit")
            || lower.contains("pearl")
            || lower.contains("percussion")
        {
            for &(pitch, name) in GM_DRUM_MAP {
                mapped_notes.push(JamSeqNoteName::new(pitch, name));
            }
        }

        let buffers = AudioBuffers::new(2, 2, block_size, sample_rate);

        Ok(Self {
            path: path.to_string(),
            plugin_name,
            plugin: Arc::new(Mutex::new(plugin)),
            buffers: Arc::new(Mutex::new(buffers)),
            params: Arc::new(Mutex::new(params)),
            param_index: Arc::new(Mutex::new(param_index)),
            note_names: Arc::new(Mutex::new(mapped_notes)),
            sample_rate,
        })
    }

    pub fn process(
        &mut self,
        inputs: Option<&JamSeqAudioBuffer>,
        outputs: &mut JamSeqAudioBuffer,
        midi_events: &[JamSeqMidiEvent],
    ) {
        if outputs.num_channels == 0 || outputs.num_samples == 0 || outputs.channel_data.is_null()
        {
            return;
        }

        let num_samples = outputs.num_samples as usize;
        let mut buffers = self.buffers.lock().unwrap();
        if buffers.block_size < num_samples {
            *buffers = AudioBuffers::new(2, 2, num_samples, self.sample_rate);
        }

        buffers.clear();

        // 1. Copy inputs if provided
        if let Some(in_buf) = inputs {
            if in_buf.num_channels > 0 && in_buf.num_samples > 0 && !in_buf.channel_data.is_null()
            {
                unsafe {
                    let in_l_ptr = *in_buf.channel_data.add(0);
                    let in_l_slice = std::slice::from_raw_parts(in_l_ptr, num_samples);
                    if buffers.inputs.len() > 0 {
                        buffers.inputs[0][..num_samples].copy_from_slice(in_l_slice);
                    }

                    if in_buf.num_channels > 1 && buffers.inputs.len() > 1 {
                        let in_r_ptr = *in_buf.channel_data.add(1);
                        let in_r_slice = std::slice::from_raw_parts(in_r_ptr, num_samples);
                        buffers.inputs[1][..num_samples].copy_from_slice(in_r_slice);
                    }
                }
            }
        }

        // 2. Dispatch MIDI events & render audio
        if let Ok(mut plugin) = self.plugin.lock() {
            for ev in midi_events {
                let status_type = ev.status & 0xF0;
                let note = ev.data1;
                let vel = ev.data2;

                if status_type == 0x90 && vel > 0 {
                    let _ = plugin.send_midi_note(note, vel, MidiChannel::Ch1);
                } else if status_type == 0x80 || (status_type == 0x90 && vel == 0) {
                    let _ = plugin.send_midi_note_off(note, MidiChannel::Ch1);
                } else if status_type == 0xB0 {
                    let _ = plugin.send_midi_cc(note, vel, MidiChannel::Ch1);
                }
            }

            let _ = plugin.process_audio(&mut buffers);
        }

        // 3. Write outputs
        unsafe {
            let out_l_ptr = *outputs.channel_data.add(0);
            let out_l_slice = std::slice::from_raw_parts_mut(out_l_ptr, num_samples);

            if buffers.outputs.len() > 0 {
                out_l_slice.copy_from_slice(&buffers.outputs[0][..num_samples]);
            }

            if outputs.num_channels > 1 {
                let out_r_ptr = *outputs.channel_data.add(1);
                let out_r_slice = std::slice::from_raw_parts_mut(out_r_ptr, num_samples);
                if buffers.outputs.len() > 1 {
                    out_r_slice.copy_from_slice(&buffers.outputs[1][..num_samples]);
                } else if buffers.outputs.len() == 1 {
                    out_r_slice.copy_from_slice(&buffers.outputs[0][..num_samples]);
                }
            }
        }
    }

    pub fn get_parameters(&self) -> *mut JamSeqParameterList {
        let params = self.params.lock().unwrap();
        let items: Vec<(String, String, String, f32, f32, f32, f32, JamSeqParamType)> = params
            .iter()
            .map(|p| {
                (
                    p.id.to_string(),
                    p.name.clone(),
                    p.group.clone(),
                    p.min,
                    p.max,
                    p.default,
                    p.value,
                    p.param_type,
                )
            })
            .collect();

        allocate_c_parameter_list(&items)
    }

    pub fn set_parameter(&self, param_id_or_name: &str, value: f32) {
        let mut params = self.params.lock().unwrap();
        let index_map = self.param_index.lock().unwrap();

        let idx = if let Ok(numeric_id) = param_id_or_name.parse::<u32>() {
            params.iter().position(|p| p.id == numeric_id)
        } else {
            index_map
                .get(param_id_or_name)
                .or_else(|| index_map.get(&param_id_or_name.to_lowercase()))
                .or_else(|| index_map.get(&param_id_or_name.to_lowercase().replace(' ', "_")))
                .or_else(|| index_map.get(&param_id_or_name.to_lowercase().replace('_', " ")))
                .copied()
        };

        if let Some(i) = idx {
            if let Some(param) = params.get_mut(i) {
                let clamped = value.clamp(param.min, param.max);
                param.value = clamped;
                let id = param.id;
                if let Ok(mut plugin) = self.plugin.lock() {
                    let _ = plugin.set_parameter(id, clamped as f64);
                }
            }
        }
    }

    pub fn get_note_names(&self) -> *mut JamSeqNoteNameList {
        let notes = self.note_names.lock().unwrap();
        allocate_c_note_name_list(&notes)
    }
}

impl Drop for Vst3Instance {
    fn drop(&mut self) {
        if let Ok(mut plugin) = self.plugin.lock() {
            let _ = plugin.stop_processing();
        }
    }
}
