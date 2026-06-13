use michiu_guard::{Unvalidated, Validate as MichiuValidate, Validated};
use nutype::nutype;

#[nutype(
    validate(not_empty, len_char_max = 20),
    derive(Debug, PartialEq, Eq, Clone)
)]
pub struct Username(String);

#[nutype(
    validate(not_empty, len_char_max = 100),
    derive(Debug, PartialEq, Eq, Clone)
)]
pub struct Email(String);

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RawRegistrationForm {
    pub username: String,
    pub email: String,
    pub password: String,
    pub password_confirm: String,
}

// Pattern A: TryFrom
impl MichiuValidate for RawRegistrationForm {
    type Error = &'static str;

    fn validate(self) -> Result<Self, Self::Error> {
        if self.password != self.password_confirm {
            return Err("Passwords do not match");
        }

        Username::try_new(self.username.clone()).map_err(|_| "Invalid username format")?;
        Email::try_new(self.email.clone()).map_err(|_| "Invalid email format")?;

        Ok(self)
    }
}

fn main() {
    println!("=== Running Pattern A: Auto-validation with TryFrom (nutype) ===");
    run_pattern_a();

    println!(
        "\n=== Running Pattern B: On-the-fly preprocessing with map & validate_with (nutype) ==="
    );
    run_pattern_b();
}

fn run_pattern_a() {
    let raw_input = Unvalidated::new(RawRegistrationForm {
        username: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        password: "secret123".to_string(),
        password_confirm: "secret123_typo".to_string(),
    });

    let result: Result<Validated<RawRegistrationForm>, _> = raw_input.try_into();

    if let Err(err_msg) = result {
        println!("Pattern A failed as expected: {}", err_msg);
    }
}

// Pattern B: use `map` and `validate_with`
fn run_pattern_b() {
    let raw_input = Unvalidated::new(RawRegistrationForm {
        username: "  alice  ".to_string(),
        email: "  ALICE@EXAMPLE.COM  ".to_string(),
        password: "secret123".to_string(),
        password_confirm: "secret123".to_string(),
    });

    let validated = raw_input
        .map(|mut form| {
            form.username = form.username.trim().to_string();
            form.email = form.email.trim().to_lowercase();
            form
        })
        .validate_with(|form| {
            if form.password != form.password_confirm {
                return Err("Passwords do not match");
            }
            Username::try_new(form.username.clone()).map_err(|_| "Invalid username format")?;
            Email::try_new(form.email.clone()).map_err(|_| "Invalid email format")?;
            Ok(form)
        });

    assert!(validated.is_ok());

    let cleaned_form = validated.unwrap().into_inner();
    assert_eq!(cleaned_form.username, "alice");
    assert_eq!(cleaned_form.email, "alice@example.com");
    println!("Pattern B succeeded! Cleaned email: {}", cleaned_form.email);
}
