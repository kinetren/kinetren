## 1. The door

- [x] 1.1 `kt_plugin_api::compute`: `ComputeHost`, `Job`, `install`, `host`
- [x] 1.2 kt-gpu: the compute host on its own device; a refused shader is an error
- [x] 1.3 Install it in kt-app and kt-cli
- [x] 1.4 t1502 the job runs and reads back; t1503 a refused shader is an error, not a panic

## 2. Datamosh

- [x] 2.1 The encoder's shaders in `fx-video-codec`; `encode` goes through the door; the CPU text goes
- [x] 2.2 With no host the stage leaves the frame and the panel says why
- [x] 2.3 The Datamosh tests run on the host; t1504 the encoder's quality; t1505 the frame without a device

## 3. Records

- [x] 3.1 Specs, crate CLAUDE.md files, F-EXT-004, What's new
- [x] 3.2 Tests of the touched crates, clippy, target-prune, commit
