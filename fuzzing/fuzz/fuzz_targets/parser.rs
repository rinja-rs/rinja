#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = <fuzz::parser::Scenario as fuzz::Scenario>::fuzz(data);
});
