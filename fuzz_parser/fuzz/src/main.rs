#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Ok(scenario) = fuzz_parser::Scenario::new(data) {
        let _ = scenario.run();
    }
});
