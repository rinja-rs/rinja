#![no_main]

use fuzz::Scenario;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Ok(scenario) = fuzz::parser::Scenario::new(data) {
        let _ = scenario.run();
    }
});
