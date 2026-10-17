use jamseq_plugin_api::{
    AudioPluginInstance, JamSeqGetHostAdapterFn, JamSeqMidiEvent, JamSeqPluginType,
    PluginHostAdapter, StackAudioOutputView, JAMSEQ_API_VERSION, JAMSEQ_ENTRYPOINT_NAME,
};
use jamseq_vst3::VST3_ADAPTER_VTABLE;
use std::path::Path;

#[test]
fn test_vst3_vtable_metadata_and_format() {
    assert_eq!(VST3_ADAPTER_VTABLE.api_version, JAMSEQ_API_VERSION);

    let adapter = PluginHostAdapter::new(&VST3_ADAPTER_VTABLE).expect("Valid API version");
    assert_eq!(adapter.format_id(), "vst3");

    assert!(adapter.handles("Surge XT.vst3"));
    assert!(adapter.handles("/usr/lib/vst3/Surge XT.vst3"));
    assert!(adapter.handles("vst3:synth"));
    assert!(!adapter.handles("plugin.clap"));
    assert!(!adapter.handles("http://gareus.org/oss/lv2/avldrums"));
}

#[test]
fn test_vst3_scanning_and_classification() {
    let adapter = PluginHostAdapter::new(&VST3_ADAPTER_VTABLE).expect("Valid API version");
    let plugins = adapter.scan();

    println!("Scanned {} VST3 plugins on system", plugins.len());
    for p in &plugins {
        println!("  • Discovered: {} ({:?}) -> {}", p.name, p.plugin_type, p.uri_or_path);
    }

    if let Some(surge) = plugins.iter().find(|p| p.name == "Surge XT") {
        assert_eq!(surge.plugin_type, JamSeqPluginType::Instrument);
    }

    if let Some(surge_fx) = plugins.iter().find(|p| p.name == "Surge XT Effects") {
        assert_eq!(surge_fx.plugin_type, JamSeqPluginType::AudioEffect);
    }
}

#[test]
fn test_vst3_instance_playback_params_and_lifecycle() {
    let adapter = PluginHostAdapter::new(&VST3_ADAPTER_VTABLE).expect("Valid API version");

    let candidate_paths = [
        "/usr/lib/vst3/Surge XT.vst3",
        "/usr/lib/vst3/3BandEQ.vst3",
        "/usr/lib/vst3/MVerb.vst3",
        "/usr/lib/vst3/Nekobi.vst3",
    ];

    let valid_path = candidate_paths.iter().find(|p| Path::new(p).exists());
    if let Some(&path) = valid_path {
        println!("Testing VST3 instance with plugin: {}", path);
        let mut instance = adapter
            .create_instance(path, 48000.0, 512)
            .expect("Failed to create VST3 instance");

        // Introspect parameters
        let params = instance.get_parameters();
        println!("Introspected {} parameters", params.len());
        assert!(!params.is_empty(), "Plugin should expose parameters");

        // Set parameter
        let first_param = &params[0];
        let test_val = (first_param.min + first_param.max) / 2.0;
        instance.set_parameter(&first_param.id, test_val);

        // Introspect note names
        let note_names = instance.get_note_names();
        println!("Introspected {} note names", note_names.len());

        // Process a block of audio and MIDI
        let mut out_l = vec![0.0f32; 256];
        let mut out_r = vec![0.0f32; 256];
        let mut out_chans: [&mut [f32]; 2] = [&mut out_l, &mut out_r];
        let mut out_view = StackAudioOutputView::<2>::new(&mut out_chans);

        let midi_events = vec![
            JamSeqMidiEvent::note_on(0, 0, 60, 100),
            JamSeqMidiEvent::note_off(128, 0, 60),
        ];

        instance.process(None, out_view.as_raw_mut(), &midi_events);
        println!("Process block complete without panic or crash");

        // Drop instance explicitly
        drop(instance);
        println!("Instance dropped cleanly");
    } else {
        println!("No candidate system VST3 plugins found at /usr/lib/vst3; skipping instance test");
    }
}

#[test]
fn test_dynamic_cdylib_loading_via_libloading() {
    let dylib_path = if cfg!(target_os = "linux") {
        "../target/debug/libjamseq_vst3.so"
    } else if cfg!(target_os = "macos") {
        "../target/debug/libjamseq_vst3.dylib"
    } else {
        "../target/debug/jamseq_vst3.dll"
    };

    if !Path::new(dylib_path).exists() {
        println!("Dylib not yet built at {}; skipping dynamic loading test", dylib_path);
        return;
    }

    unsafe {
        let lib = libloading::Library::new(dylib_path)
            .expect("Failed to dynamically load libjamseq_vst3 shared library");

        let entrypoint: libloading::Symbol<JamSeqGetHostAdapterFn> = lib
            .get(JAMSEQ_ENTRYPOINT_NAME.as_bytes())
            .expect("Failed to locate jamseq_get_host_adapter symbol");

        let vtable_ptr = entrypoint();
        assert!(!vtable_ptr.is_null());

        let vtable = &*vtable_ptr;
        assert_eq!(vtable.api_version, JAMSEQ_API_VERSION);

        let adapter = PluginHostAdapter::new(vtable).expect("Valid API version");
        assert_eq!(adapter.format_id(), "vst3");
        assert!(adapter.handles("test.vst3"));
        println!("✓ Successfully loaded and verified libjamseq_vst3.so via libloading");
    }
}
