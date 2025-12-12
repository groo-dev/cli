use console::Style;

const COLORS: &[fn() -> Style] = &[
    || Style::new().cyan(),
    || Style::new().magenta(),
    || Style::new().yellow(),
    || Style::new().green(),
    || Style::new().blue(),
    || Style::new().red(),
    || Style::new().cyan().bold(),
    || Style::new().magenta().bold(),
    || Style::new().yellow().bold(),
    || Style::new().green().bold(),
];

pub fn get_color_for_index(index: usize) -> Style {
    COLORS[index % COLORS.len()]()
}

pub fn format_log_line(service_name: &str, line: &str, color: &Style) -> String {
    let prefix = color.apply_to(format!("[{}]", service_name));
    format!("{} {}", prefix, line)
}

pub fn print_service_log(service_name: &str, line: &str, color: &Style) {
    println!("{}", format_log_line(service_name, line, color));
}

pub fn print_service_error(service_name: &str, line: &str, color: &Style) {
    eprintln!("{}", format_log_line(service_name, line, color));
}
