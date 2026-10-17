# `jamseq-vst3`

**Standalone dynamic VST3 plugin host adapter for Jamband.**

---

## 1. Overview

`jamseq-vst3` is an independent dynamic shared library (`libjamseq_vst3.so` on Linux, `libjamseq_vst3.dylib` on macOS, and `jamseq_vst3.dll` on Windows) implementing the binary-stable `JamSeqAdapterVTable` C-ABI contract defined by `jamseq-plugin-api`.

It completely isolates Steinberg VST3 COM hosting (`vst3-host`) and COM bindings from the core `jamseq` audio engine.

---

## 2. Capabilities

- **Discovery & Scanning**: Scans standard VST3 paths (`/usr/lib/vst3`, `~/.vst3`, `VST3_PATH`, etc.) and automatically categorizes plugins into `Instrument`, `AudioEffect`, or `Analyzer`.
- **Instance Lifecycle & Real-Time DSP**: Sample-accurate block rendering with multi-channel audio buffers and frame-accurate MIDI events.
- **Parameter Introspection & Modulation**: Full enumeration of plugin parameters, value formatting, and real-time updates.
- **Note Name Introspection**: Mapped drum and note names with automatic fallback to the General MIDI Level 1 drum standard for percussion instruments.
- **Dynamic C-ABI Export**: Exports `jamseq_get_host_adapter` to enable dynamic loading via `libloading`.

---

## 3. Verification

Run the test suite:
```bash
cargo test -p jamseq-vst3
```
