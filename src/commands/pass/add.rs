use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use console::style;
use dialoguer::{Confirm, Input, Password, Select};
use rand::Rng;
use uuid::Uuid;

use crate::auth::provider;
use crate::pass::client::PassClient;
use crate::pass::types::{
    BankAccountItem, BankAccountType, CardItem, NoteItem, PasswordItem, TotpAlgorithm,
    TotpConfig, VaultItem,
};

const UPPERCASE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWERCASE: &str = "abcdefghijklmnopqrstuvwxyz";
const NUMBERS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*()_+-=[]{}|;:,.<>?";

pub async fn run() -> Result<()> {
    // Check auth
    let auth = provider::get_valid_auth().await?;
    let master_password = rpassword::prompt_password("🔑 Master password: ")?;

    // Select item type
    let item_types = vec!["Password", "Note", "Card", "Bank Account"];
    let selection = Select::new()
        .with_prompt("What type of item do you want to add?")
        .items(&item_types)
        .default(0)
        .interact()?;

    println!("{}", style("Unlocking vault...").dim());

    // Unlock vault
    let client = PassClient::new(auth.access_token);
    let (mut vault, key, version) = client.unlock(&master_password).await?;

    // Create item based on type
    let item = match selection {
        0 => create_password_item()?,
        1 => create_note_item()?,
        2 => create_card_item()?,
        3 => create_bank_account_item()?,
        _ => unreachable!(),
    };

    let item_name = item.name().to_string();
    let item_type = item.type_label();

    // Add to vault
    vault.items.push(item);
    vault.last_modified = now_timestamp();

    // Sync to server
    println!("{}", style("Saving to vault...").dim());
    client.update_vault(&vault, &key, version).await?;

    println!(
        "{} {} {} added to vault",
        style("✓").green(),
        item_type,
        style(&item_name).cyan()
    );

    Ok(())
}

fn create_password_item() -> Result<VaultItem> {
    println!("\n{}", style("New Password").bold());

    let name: String = Input::new()
        .with_prompt("Name")
        .interact_text()?;

    let username: String = Input::new()
        .with_prompt("Username/Email")
        .allow_empty(true)
        .interact_text()?;

    // Password: generate or enter manually
    let password = prompt_password()?;

    let url: String = Input::new()
        .with_prompt("URL (optional)")
        .allow_empty(true)
        .interact_text()?;

    let urls = if url.is_empty() {
        vec![]
    } else {
        vec![url]
    };

    let notes: String = Input::new()
        .with_prompt("Notes (optional)")
        .allow_empty(true)
        .interact_text()?;

    // TOTP setup
    let totp = if Confirm::new()
        .with_prompt("Add TOTP (2FA)?")
        .default(false)
        .interact()?
    {
        Some(prompt_totp()?)
    } else {
        None
    };

    let now = now_timestamp();

    Ok(VaultItem::Password(PasswordItem {
        id: Uuid::new_v4().to_string(),
        name,
        username,
        password,
        urls,
        notes: if notes.is_empty() { None } else { Some(notes) },
        totp,
        folder_id: None,
        favorite: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    }))
}

fn create_note_item() -> Result<VaultItem> {
    println!("\n{}", style("New Secure Note").bold());

    let name: String = Input::new()
        .with_prompt("Name")
        .interact_text()?;

    println!("{}", style("Enter note content (press Enter twice to finish):").dim());

    let mut content = String::new();
    let mut empty_count = 0;

    loop {
        let line: String = Input::new()
            .with_prompt("")
            .allow_empty(true)
            .interact_text()?;

        if line.is_empty() {
            empty_count += 1;
            if empty_count >= 2 {
                break;
            }
            content.push('\n');
        } else {
            empty_count = 0;
            content.push_str(&line);
            content.push('\n');
        }
    }

    let content = content.trim().to_string();

    if content.is_empty() {
        return Err(anyhow!("Note content cannot be empty"));
    }

    let now = now_timestamp();

    Ok(VaultItem::Note(NoteItem {
        id: Uuid::new_v4().to_string(),
        name,
        content,
        folder_id: None,
        favorite: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    }))
}

fn create_card_item() -> Result<VaultItem> {
    println!("\n{}", style("New Card").bold());

    let name: String = Input::new()
        .with_prompt("Name (e.g., 'Chase Visa')")
        .interact_text()?;

    let cardholder_name: String = Input::new()
        .with_prompt("Cardholder name")
        .interact_text()?;

    let number: String = Input::new()
        .with_prompt("Card number")
        .validate_with(|input: &String| {
            let digits: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() >= 13 && digits.len() <= 19 {
                Ok(())
            } else {
                Err("Card number must be 13-19 digits")
            }
        })
        .interact_text()?;

    // Normalize card number (remove spaces/dashes)
    let number: String = number.chars().filter(|c| c.is_ascii_digit()).collect();
    let brand = detect_card_brand(&number);

    let exp_month: String = Input::new()
        .with_prompt("Expiry month (MM)")
        .validate_with(|input: &String| {
            match input.parse::<u8>() {
                Ok(m) if (1..=12).contains(&m) => Ok(()),
                _ => Err("Month must be 01-12"),
            }
        })
        .interact_text()?;

    let exp_year: String = Input::new()
        .with_prompt("Expiry year (YY or YYYY)")
        .validate_with(|input: &String| {
            match input.parse::<u16>() {
                Ok(y) if (24..=99).contains(&y) || (2024..=2099).contains(&y) => Ok(()),
                _ => Err("Year must be valid (e.g., 25 or 2025)"),
            }
        })
        .interact_text()?;

    let cvv: String = Password::new()
        .with_prompt("CVV")
        .interact()?;

    let notes: String = Input::new()
        .with_prompt("Notes (optional)")
        .allow_empty(true)
        .interact_text()?;

    let now = now_timestamp();

    Ok(VaultItem::Card(CardItem {
        id: Uuid::new_v4().to_string(),
        name,
        cardholder_name,
        number,
        exp_month: format!("{:0>2}", exp_month),
        exp_year: if exp_year.len() == 2 {
            format!("20{}", exp_year)
        } else {
            exp_year
        },
        cvv,
        brand,
        notes: if notes.is_empty() { None } else { Some(notes) },
        folder_id: None,
        favorite: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    }))
}

fn create_bank_account_item() -> Result<VaultItem> {
    println!("\n{}", style("New Bank Account").bold());

    let name: String = Input::new()
        .with_prompt("Name (e.g., 'Main Checking')")
        .interact_text()?;

    let bank_name: String = Input::new()
        .with_prompt("Bank name")
        .interact_text()?;

    let account_types = vec!["Checking", "Savings", "Other"];
    let type_idx = Select::new()
        .with_prompt("Account type")
        .items(&account_types)
        .default(0)
        .interact()?;

    let account_type = match type_idx {
        0 => BankAccountType::Checking,
        1 => BankAccountType::Savings,
        _ => BankAccountType::Other,
    };

    let account_number: String = Input::new()
        .with_prompt("Account number")
        .interact_text()?;

    let routing_number: String = Input::new()
        .with_prompt("Routing number (optional)")
        .allow_empty(true)
        .interact_text()?;

    let iban: String = Input::new()
        .with_prompt("IBAN (optional)")
        .allow_empty(true)
        .interact_text()?;

    let swift_bic: String = Input::new()
        .with_prompt("SWIFT/BIC (optional)")
        .allow_empty(true)
        .interact_text()?;

    let notes: String = Input::new()
        .with_prompt("Notes (optional)")
        .allow_empty(true)
        .interact_text()?;

    let now = now_timestamp();

    Ok(VaultItem::BankAccount(BankAccountItem {
        id: Uuid::new_v4().to_string(),
        name,
        bank_name,
        account_type,
        account_number,
        routing_number: if routing_number.is_empty() {
            None
        } else {
            Some(routing_number)
        },
        iban: if iban.is_empty() { None } else { Some(iban) },
        swift_bic: if swift_bic.is_empty() {
            None
        } else {
            Some(swift_bic)
        },
        notes: if notes.is_empty() { None } else { Some(notes) },
        folder_id: None,
        favorite: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    }))
}

fn prompt_password() -> Result<String> {
    let options = vec!["Generate password", "Enter manually"];
    let choice = Select::new()
        .with_prompt("Password")
        .items(&options)
        .default(0)
        .interact()?;

    if choice == 0 {
        // Generate password
        let length: usize = Input::new()
            .with_prompt("Length")
            .default(20)
            .interact_text()?;

        let password = generate_password(length)?;
        println!("{} {}", style("Generated:").dim(), style(&password).green());
        Ok(password)
    } else {
        // Manual entry
        let password = Password::new()
            .with_prompt("Password")
            .with_confirmation("Confirm password", "Passwords don't match")
            .interact()?;
        Ok(password)
    }
}

fn prompt_totp() -> Result<TotpConfig> {
    let secret: String = Input::new()
        .with_prompt("TOTP secret (base32)")
        .interact_text()?;

    // Normalize: uppercase, remove spaces
    let secret = secret
        .to_uppercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();

    let algorithms = vec!["SHA1 (most common)", "SHA256", "SHA512"];
    let algo_idx = Select::new()
        .with_prompt("Algorithm")
        .items(&algorithms)
        .default(0)
        .interact()?;

    let algorithm = match algo_idx {
        0 => TotpAlgorithm::SHA1,
        1 => TotpAlgorithm::SHA256,
        _ => TotpAlgorithm::SHA512,
    };

    let digits: u8 = Input::new()
        .with_prompt("Digits")
        .default(6)
        .interact_text()?;

    let period: u32 = Input::new()
        .with_prompt("Period (seconds)")
        .default(30)
        .interact_text()?;

    Ok(TotpConfig {
        secret,
        algorithm,
        digits,
        period,
    })
}

fn generate_password(length: usize) -> Result<String> {
    let mut rng = rand::thread_rng();
    let mut charset = String::new();
    let mut required: Vec<char> = Vec::new();

    charset.push_str(UPPERCASE);
    required.push(
        UPPERCASE
            .chars()
            .nth(rng.gen_range(0..UPPERCASE.len()))
            .unwrap(),
    );

    charset.push_str(LOWERCASE);
    required.push(
        LOWERCASE
            .chars()
            .nth(rng.gen_range(0..LOWERCASE.len()))
            .unwrap(),
    );

    charset.push_str(NUMBERS);
    required.push(
        NUMBERS
            .chars()
            .nth(rng.gen_range(0..NUMBERS.len()))
            .unwrap(),
    );

    charset.push_str(SYMBOLS);
    required.push(
        SYMBOLS
            .chars()
            .nth(rng.gen_range(0..SYMBOLS.len()))
            .unwrap(),
    );

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

fn detect_card_brand(number: &str) -> Option<String> {
    let brand = if number.starts_with('4') {
        "Visa"
    } else if number.starts_with("51")
        || number.starts_with("52")
        || number.starts_with("53")
        || number.starts_with("54")
        || number.starts_with("55")
    {
        "Mastercard"
    } else if number.starts_with("34") || number.starts_with("37") {
        "American Express"
    } else if number.starts_with("6011") || number.starts_with("65") {
        "Discover"
    } else {
        return None;
    };
    Some(brand.to_string())
}

fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
