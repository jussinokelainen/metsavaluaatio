use color_eyre::Result;

/// A struct to pass valid flags to the parser.
///
/// flags: normal flags, basically like booleans.
/// valued_flags: flags that take a value after them, for example '--depth 1'
/// optional_flags: flags that try to take a value with them, but also work without
///
/// Optional flags will take the argument after them as it's value unless they are
/// the last argument, or the argument after them is another flag (starts with '-')
pub struct Flagset {
    pub flags: Vec<&'static str>,
    pub value_flags: Vec<&'static str>,
    pub opt_flags: Vec<&'static str>,
}

#[allow(dead_code)]
impl Flagset {
    pub fn new() -> Self {
        Self {
            flags: Vec::new(),
            value_flags: Vec::new(),
            opt_flags: Vec::new(),
        }
    }
}

/// flags: normal flags and optional flags that didn't get a value with them
/// valued_flags: tuple that contains the flag, and it's value
/// normal_str: strings that were in the arguments, strings that were the values
/// of optional or value flags are not contained here.
pub struct Arguments {
    pub flags: Vec<String>,
    pub valued_flags: Vec<(String, String)>,
    pub normal_str: Vec<String>,
}

/// A function to parse arguments, returns them as a struct with separate vectors
/// for normal flags, flags that take values and normal strings.
///
/// As a parameter requires  a Flagset struct that contains valid arguments, and
/// optionally a vector of custom arguments, instead of taking them from std::env
/// If all flags are to be accepted, all structs in the given Flagset struct
/// must be empty. If that is the case, all flags will be parsed and returned
/// as normal boolean-style flags
///
/// Returns a Result<Arguments> which is an error type if there was invalid flags
pub fn parse_args(input: Flagset, custom_args: Option<Vec<String>>) -> Result<Arguments> {
    let mut only_parse_as_normal = false;
    let input_args = match custom_args {
        Some(a) => a,
        None => {
            // Get all arguments
            let mut args: Vec<String> = std::env::args().collect();

            // Remove the program name from arguments before returning
            args.remove(0);
            args
        }
    };

    let mut accept_all = false;
    let mut flags = Arguments {
        flags: vec![],
        valued_flags: vec![],
        normal_str: vec![],
    };

    // If no specific flags are given by the caller, all flags will be valid
    if input.flags.is_empty() && input.value_flags.is_empty() && input.opt_flags.is_empty() {
        accept_all = true;
    }

    // Check if a flag takes a value as a parameter
    let requires_value = |flag: &String, num: usize| -> bool {
        if input.value_flags.contains(&flag.as_str()) {
            return true;
        }

        if input.opt_flags.contains(&flag.as_str())
            && (num + 1) < input_args.len()
            && !input_args[num + 1].starts_with('-')
        {
            return true;
        }

        false
    };

    // Handles a single flag, only returns an error when an invalid flag is given
    let mut handle_flag = |flag: String, value: Option<&str>, num: &mut usize| -> Result<()> {
        if requires_value(&flag, *num) {
            match value {
                Some(v) => {
                    // Flag requires a value, and is of type --foo=bar
                    // so use given value
                    let new_flag = (flag, v.to_string());
                    flags.valued_flags.push(new_flag);
                    Ok(())
                }
                None => {
                    // Flag requires the next argument as its value, check that it is not
                    // the last argument before taking the value
                    if (*num + 1) < input_args.len() {
                        // Get the next argument to be the value for this flag
                        let new_flag = (flag, input_args[*num + 1].clone());
                        flags.valued_flags.push(new_flag);
                        *num += 1;
                        Ok(())
                    } else {
                        Err(color_eyre::eyre::eyre!("Invalid flag: {}", flag))
                    }
                }
            }
        } else {
            match value {
                Some(_) => Err(color_eyre::eyre::eyre!(
                    "Value given for a non-valued flag: {}",
                    flag
                )),
                None => {
                    if input.flags.contains(&flag.as_str())
                        || input.opt_flags.contains(&flag.as_str())
                        || accept_all
                    {
                        flags.flags.push(flag.clone().to_string());
                        Ok(())
                    } else {
                        Err(color_eyre::eyre::eyre!("Invalid flag: {}", flag))
                    }
                }
            }
        }
    };

    let mut count = 0;
    while count < input_args.len() {
        if input_args[count] == "--" {
            only_parse_as_normal = true;
            count += 1;
            continue;
        }
        if only_parse_as_normal {
            flags.normal_str.push(input_args[count].clone());
        } else if input_args[count].contains('=') {
            if input_args[count].starts_with("--") {
                let flag_with_value = &input_args[count][2..];
                if let Some((flag, value)) = flag_with_value.split_once('=') {
                    handle_flag(flag.to_string(), Some(value), &mut count)?;
                }
            } else if input_args[count].starts_with('-') {
                let full_flag = &input_args[count][1..];
                if let Some(equals_pos) = full_flag.find('=') {
                    let last_flag_pos = equals_pos - 1;
                    // Handle flags that were not the last one as normal ones
                    // e.g. --abc=foo -> a and b as normal, c as c=foo
                    for i in 0..last_flag_pos {
                        if let Some(flag) = full_flag.chars().nth(i) {
                            handle_flag(flag.to_string(), None, &mut count)?;
                        }
                    }

                    // Handle the last character before the '='
                    // as the flag that takes the string after the '=' as its value
                    if let Some(val_flag) = full_flag.chars().nth(last_flag_pos) {
                        if let Some((_, value)) = full_flag.split_once('=') {
                            handle_flag(val_flag.to_string(), Some(value), &mut count)?;
                        }
                    }
                }
            } else {
                flags.normal_str.push(input_args[count].clone());
            }
        } else {
            if input_args[count].starts_with("--") {
                let flag = &input_args[count][2..];
                handle_flag(flag.to_string(), None, &mut count)?;
            } else if input_args[count].starts_with('-') {
                let flag = &input_args[count][1..];
                for c in flag.chars() {
                    handle_flag(c.to_string(), None, &mut count)?;
                }
            } else {
                flags.normal_str.push(input_args[count].clone());
            }
        }
        count += 1;
    }

    Ok(flags)
}
