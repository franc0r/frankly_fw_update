///
/// Build Device Simulator API
///
fn main() {
    cc::Build::new()
        .cpp(true)
        .include("./../../frankly-bootloader/include")
        .include("./../../frankly-bootloader/utils/device_sim_api/include")
        .file("./../../frankly-bootloader/src/francor/franklyboot/msg.cpp")
        .file("./../../frankly-bootloader/utils/device_sim_api/src/device_sim_api.cpp")
        .cpp_link_stdlib("stdc++")
        .compile("libfranklyboot-device-sim-api.a");
}
