use anyhow::{anyhow, Result};
use console::style;
use rand::Rng;

const UPPERCASE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWERCASE: &str = "abcdefghijklmnopqrstuvwxyz";
const NUMBERS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*()_+-=[]{}|;:,.<>?";

// EFF short wordlist (subset for CLI)
const WORDLIST: &[&str] = &[
    "acid", "acorn", "acre", "acts", "afar", "affix", "aged", "agent", "agile", "aging",
    "agony", "ahead", "aide", "aids", "aim", "ajar", "alarm", "album", "alert", "alike",
    "alive", "alley", "allot", "allow", "ally", "aloft", "alone", "amend", "amino", "ample",
    "amuse", "angel", "anger", "angle", "ankle", "apart", "apex", "apple", "apply", "apron",
    "arena", "argue", "arise", "armor", "army", "aroma", "array", "arrow", "arson", "art",
    "ashen", "aside", "asset", "atom", "attic", "audio", "aunt", "avid", "avoid", "awake",
    "award", "awash", "awful", "axis", "bacon", "badge", "badly", "bagel", "baggy", "baked",
    "baker", "balmy", "banjo", "barge", "barn", "bash", "basic", "batch", "bath", "baton",
    "blade", "blame", "bland", "blank", "blast", "blaze", "bleak", "blend", "bless", "blimp",
    "blind", "blink", "bliss", "blitz", "block", "blond", "blood", "bloom", "blown", "bluff",
    "blunt", "blurt", "blush", "board", "boast", "boat", "body", "bogus", "boil", "bolt",
    "bonus", "bony", "book", "booth", "boots", "boss", "botch", "both", "boxer", "brace",
    "brain", "brake", "brand", "brass", "brave", "bravo", "bread", "break", "breed", "brick",
    "bride", "brief", "bring", "brink", "brisk", "broad", "broil", "broke", "brook", "broom",
    "brunt", "brush", "buck", "buddy", "budge", "buggy", "build", "built", "bulge", "bulk",
    "bully", "bunch", "bunny", "Burke", "burnt", "burst", "bury", "bush", "bust", "busy",
    "buyer", "bylaw", "cabin", "cable", "cache", "cadet", "cage", "cake", "calm", "camel",
    "camp", "canal", "candy", "cane", "canon", "cape", "card", "cargo", "carol", "carry",
    "carve", "case", "cash", "cast", "catch", "cause", "cave", "cease", "cedar", "chain",
    "chair", "champ", "chant", "chaos", "charm", "chart", "chase", "cheap", "cheat", "check",
];

#[allow(clippy::too_many_arguments)]
pub async fn run(
    length: usize,
    no_uppercase: bool,
    no_lowercase: bool,
    no_numbers: bool,
    no_symbols: bool,
    passphrase: bool,
    words: usize,
    separator: &str,
    print: bool,
) -> Result<()> {
    let generated = if passphrase {
        generate_passphrase(words, separator)
    } else {
        generate_password(length, !no_uppercase, !no_lowercase, !no_numbers, !no_symbols)?
    };

    if print {
        println!("{}", generated);
    } else {
        copy_to_clipboard(&generated)?;
        println!(
            "{} {}",
            style("✓").green(),
            style(format!("Generated and copied: {}", generated)).bold()
        );
    }

    Ok(())
}

fn generate_password(
    length: usize,
    use_uppercase: bool,
    use_lowercase: bool,
    use_numbers: bool,
    use_symbols: bool,
) -> Result<String> {
    let mut rng = rand::thread_rng();
    let mut charset = String::new();
    let mut required: Vec<char> = Vec::new();

    if use_uppercase {
        charset.push_str(UPPERCASE);
        required.push(UPPERCASE.chars().nth(rng.gen_range(0..UPPERCASE.len())).unwrap());
    }
    if use_lowercase {
        charset.push_str(LOWERCASE);
        required.push(LOWERCASE.chars().nth(rng.gen_range(0..LOWERCASE.len())).unwrap());
    }
    if use_numbers {
        charset.push_str(NUMBERS);
        required.push(NUMBERS.chars().nth(rng.gen_range(0..NUMBERS.len())).unwrap());
    }
    if use_symbols {
        charset.push_str(SYMBOLS);
        required.push(SYMBOLS.chars().nth(rng.gen_range(0..SYMBOLS.len())).unwrap());
    }

    if charset.is_empty() {
        return Err(anyhow!("At least one character set must be enabled"));
    }

    let charset: Vec<char> = charset.chars().collect();
    let mut password: Vec<char> = (0..length)
        .map(|_| charset[rng.gen_range(0..charset.len())])
        .collect();

    // Ensure at least one character from each required set
    for (i, c) in required.into_iter().enumerate() {
        if i < password.len() {
            let pos = rng.gen_range(0..password.len());
            password[pos] = c;
        }
    }

    Ok(password.into_iter().collect())
}

fn generate_passphrase(word_count: usize, separator: &str) -> String {
    let mut rng = rand::thread_rng();

    let words: Vec<String> = (0..word_count)
        .map(|_| {
            let word = WORDLIST[rng.gen_range(0..WORDLIST.len())];
            // Capitalize first letter
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect();

    words.join(separator)
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    use arboard::Clipboard;

    let mut clipboard = Clipboard::new().map_err(|e| anyhow!("Failed to access clipboard: {}", e))?;
    clipboard
        .set_text(text)
        .map_err(|e| anyhow!("Failed to copy to clipboard: {}", e))?;
    Ok(())
}
