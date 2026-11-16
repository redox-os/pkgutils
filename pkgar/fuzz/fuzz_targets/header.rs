#![no_main]
#[macro_use] extern crate libfuzzer_sys;
extern crate pkgar;

use std::mem;

fuzz_target!(|data: &[u8]| {
    let public_key = pkgar::PublicKey::from_data([0; 32]);
    match pkgar::Header::new(data, &public_key) {
        Ok(_) => panic!("parsed random header"),
        Err(_) => (),
    }

    match unsafe { pkgar::Header::new_unchecked(data) } {
        Ok(header) => {
            let entries_data = &data[mem::size_of::<pkgar::Header>()..];
            match header.entries(entries_data) {
                Ok(_) => panic!("parsed random entries"),
                Err(_) => (),
            }

            match unsafe { pkgar::Header::entries_unchecked(entries_data) } {
                Ok(entries) => {
                    //TODO: more tests on entries
                },
                Err(_) => (),
            }
        },
        Err(_) => ()
    }
});
