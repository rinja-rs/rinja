#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = <fuzz::filters::Scenario as fuzz::Scenario>::fuzz(data);
});
