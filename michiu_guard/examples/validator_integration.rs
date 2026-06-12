use michiu_guard::{Unvalidated, Validate as MichiuValidate, Validated};
use validator::Validate as ValidatorValidate;

#[derive(Debug, PartialEq, Eq, ValidatorValidate)]
pub struct SignupForm {
    #[validate(email)]
    pub mail: String,

    #[validate(url)]
    pub site: String,
}

// Pattern A: TryFrom
impl MichiuValidate for SignupForm {
    type Error = validator::ValidationErrors;

    fn validate(self) -> Result<Self, Self::Error> {
        ValidatorValidate::validate(&self)?;
        Ok(self)
    }
}

fn main() {
    println!("=== Running Pattern A: Auto-validation with TryFrom ===");
    run_pattern_a();

    println!("\n=== Running Pattern B: On-the-fly preprocessing with map & validate_with ===");
    run_pattern_b();
}

fn run_pattern_a() {
    let raw_input = Unvalidated::new(SignupForm {
        mail: "invalid-email".to_string(),
        site: "https://example.com".to_string(),
    });

    let result: Result<Validated<SignupForm>, _> = raw_input.try_into();

    if let Err(errors) = result {
        println!("Pattern A failed as expected:\n{:#?}", errors);
    }
}

// Pattern B: use `map` and `validate_with`
fn run_pattern_b() {
    let raw_input = Unvalidated::new(SignupForm {
        mail: "  USER@EXAMPLE.COM  ".to_string(),
        site: "https://example.com".to_string(),
    });

    let validated = raw_input
        .map(|mut form| {
            form.mail = form.mail.trim().to_lowercase();
            form
        })
        .validate_with::<SignupForm, validator::ValidationErrors>(|form| {
            ValidatorValidate::validate(&form)?;
            Ok(form)
        });

    assert!(validated.is_ok());

    let cleaned_form = validated.unwrap().into_inner();

    assert_eq!(cleaned_form.mail, "user@example.com");
    println!("Pattern B succeeded! Cleaned mail: {}", cleaned_form.mail);
}
