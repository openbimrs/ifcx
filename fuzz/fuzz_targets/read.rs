//! `IfcxFile::from_json_slice` on arbitrary bytes. A file that reads must
//! write and read back to an equal file (the reader is lossless), and the
//! whole single-file pipeline (validate, flatten, compose) must not panic.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openbim_ifcx::{compose, flatten, IfcxFile};
use openbim_ifcx_fuzz::check_composition;

fuzz_target!(|data: &[u8]| {
    let Ok(file) = IfcxFile::from_json_slice(data) else {
        return;
    };
    let text = file.to_json_string().expect("a read file writes");
    let again = IfcxFile::from_json_str(&text).expect("a written file reads");
    assert_eq!(file, again, "write then read changed the file");
    let pretty = file.to_json_string_pretty().expect("a read file writes");
    assert_eq!(IfcxFile::from_json_str(&pretty).expect("reads"), file);

    let _ = file.validate();
    if let Ok(composition) = compose(&flatten(&file.data)) {
        check_composition(&composition);
    }
});
