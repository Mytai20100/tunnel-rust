use chrono::Local;
use colored::Colorize;

pub fn log_info(message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    println!("{} {} {}",
        format!("[INFO]").green(),
        timestamp.to_string().bright_black(),
        message
    );
}

pub fn log_error(message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    println!("{} {} {}",
        format!("[ERROR]").red(),
        timestamp.to_string().bright_black(),
        message
    );
}

pub fn log_warning(message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    println!("{} {} {}",
        format!("[WARN]").yellow(),
        timestamp.to_string().bright_black(),
        message
    );
}

pub fn log_share(message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    println!("{} {} {}",
        format!("[SHARE]").bright_purple(),
        timestamp.to_string().bright_black(),
        message
    );
}

pub fn log_debug(message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    println!("{} {} {}",
        format!("[DEBUG]").cyan(),
        timestamp.to_string().bright_black(),
        message
    );
}
