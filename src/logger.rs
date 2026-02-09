// use std::fs::OpenOptions;
// use std::io::Write;

// pub fn log(msg: &str) {
//     let Ok(mut file) = OpenOptions::new()
//         .create(true)
//         .append(true)
//         .open("wlx_debug.log")
//     else {
//         return;
//     };
//     let _ = writeln!(file, "{}", msg);
// }
