//! `renderer.txt`: the adapter record a driver run leaves as evidence.

use viewer::evidence::renderer_info;

#[test]
fn an_adapter_without_a_driver_name_leaves_no_driver_line() {
    let info = renderer_info("D3D12 (NVIDIA GeForce GTX 1080 Ti)", "Gl", "Other", "");
    assert_eq!(
        "renderer: D3D12 (NVIDIA GeForce GTX 1080 Ti)\nbackend: Gl\ndevice: Other\n",
        info
    );
    assert!(info.lines().all(|l| l == l.trim_end()), "{info:?}");
}

#[test]
fn a_named_driver_is_recorded_on_its_own_line() {
    assert_eq!(
        "renderer: llvmpipe\nbackend: Vulkan\ndevice: Cpu\ndriver: Mesa\n",
        renderer_info("llvmpipe", "Vulkan", "Cpu", "Mesa")
    );
}
