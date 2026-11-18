pub fn display_name(filepath: &str, absolute: bool) -> &str {
    if absolute {
        filepath
    } else {
        filepath.rsplit('/').next().unwrap_or(filepath)
    }
}

pub fn format_time_duration(time: f64, duration: f64) -> String {
    let pos = format_time(time);
    let dur = format_time(duration);
    format!("{pos}/{dur}")
}

pub fn format_time(time: f64) -> String {
    let secs = time as i64;
    let mm = format!("{:02}", secs / 60);
    let ss = format!("{:02}", secs % 60);
    format!("{mm}:{ss}")
}
