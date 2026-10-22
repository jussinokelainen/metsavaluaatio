use color_eyre::{Result, eyre::eyre};

/// A struct to pass valid flags to the parser.
///
/// flags: normal flags, basically like booleans.
/// valued_flags: flags that take a value after them, for example '--depth 1'
/// optional_flags: flags that try to take a value with them, but also work without
///
/// Optional flags will take the argument after them as it's value unless they are
/// the last argument, or the argument after them is another flag (starts with '-')
#[derive(PartialEq)]
pub struct Flagset {
    pub flags: Option<Vec<&'static str>>,
    pub value_flags: Option<Vec<&'static str>>,
    pub opt_flags: Option<Vec<&'static str>>,
}

#[allow(dead_code)]
impl Flagset {
    pub fn new_empty() -> Self {
        Self {
            flags: None,
            value_flags: None,
            opt_flags: None,
        }
    }
}

/// flags: normal flags and optional flags that didn't get a value with them
/// valued_flags: tuple that contains the flag, and it's value
/// normal_str: strings that were in the arguments, strings that were the values
/// of optional or value flags are not contained here.
pub struct Arguments {
    pub flags: Option<Vec<String>>,
    pub value_flags: Option<Vec<(String, String)>>,
    pub normal_str: Option<Vec<String>>,
}

#[allow(dead_code)]
impl Arguments {
    fn new_empty() -> Self {
        Self {
            flags: None,
            value_flags: None,
            normal_str: None,
        }
    }
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

    let accept_all = input == Flagset::new_empty();
    let mut flags = Arguments::new_empty();

    // Check if a flag takes a value as a parameter
    let requires_value = |flag: &String, num: usize, had_value_with_equals: bool| -> bool {
        let Some(v_flags) = &input.value_flags else {
            return false;
        };

        if v_flags.contains(&flag.as_str()) {
            return true;
        }

        if let Some(o_flags) = &input.opt_flags {
            if o_flags.contains(&flag.as_str()) {
                if had_value_with_equals {
                    return true;
                }
                if (num + 1) < input_args.len() && !input_args[num + 1].starts_with('-') {
                    return true;
                }
            }
        }

        false
    };

    // Handles a single flag, only returns an error when an invalid flag is given
    let mut handle_flag = |flag: String, value: Option<&str>, num: &mut usize| -> Result<()> {
        if requires_value(&flag, *num, value.is_some()) {
            match value {
                Some(v) => {
                    // Flag requires a value, and is of type --foo=bar
                    // so use given value
                    let new_flag = (flag, v.to_string());
                    flags
                        .value_flags
                        .get_or_insert_with(Vec::new)
                        .push(new_flag);
                    Ok(())
                }
                None => {
                    // Flag requires the next argument as its value, check that it is not
                    // the last argument before taking the value
                    if (*num + 1) < input_args.len() {
                        // Get the next argument to be the value for this flag
                        let new_flag = (flag, input_args[*num + 1].clone());
                        flags
                            .value_flags
                            .get_or_insert_with(Vec::new)
                            .push(new_flag);
                        *num += 1;
                        Ok(())
                    } else {
                        Err(color_eyre::eyre::eyre!("Invalid flag: {}", flag))
                    }
                }
            }
        } else {
            match value {
                Some(_) => Err(eyre!("Value given for a non-valued flag: {}", flag)),
                None => {
                    if accept_all {
                        flags.flags.get_or_insert_with(Vec::new).push(flag);
                        Ok(())
                    } else if let Some(i_flags) = &input.flags
                        && i_flags.contains(&flag.as_str())
                    {
                        flags.flags.get_or_insert_with(Vec::new).push(flag);
                        Ok(())
                    } else if let Some(i_flags) = &input.opt_flags
                        && i_flags.contains(&flag.as_str())
                    {
                        flags.flags.get_or_insert_with(Vec::new).push(flag);
                        Ok(())
                    } else {
                        Err(eyre!("Invalid flag: {}", flag))
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
            flags
                .normal_str
                .get_or_insert_with(Vec::new)
                .push(input_args[count].to_owned());
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
                flags
                    .normal_str
                    .get_or_insert_with(Vec::new)
                    .push(input_args[count].to_owned());
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
                flags
                    .normal_str
                    .get_or_insert_with(Vec::new)
                    .push(input_args[count].to_owned());
            }
        }
        count += 1;
    }

    Ok(flags)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli() -> Flagset {
        Flagset {
            flags: Some(vec!["h", "help", "foo", "a", "b"]),
            value_flags: Some(vec!["bar", "bat", "c"]),
            opt_flags: Some(vec!["baz"]),
        }
    }

    fn args(input: &[&str]) -> Option<Vec<String>> {
        Some(input.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn only_normal_strings() {
        let parsed = parse_args(cli(), args(&["foobarbaz"])).unwrap();
        assert_eq!(parsed.normal_str, Some(vec!["foobarbaz".to_string()]));
        assert!(parsed.flags.is_none());
        assert!(parsed.value_flags.is_none());
    }

    #[test]
    fn bool_flags() {
        let long = parse_args(cli(), args(&["--help", "--foo"])).unwrap();
        assert!(long.value_flags.is_none());
        assert!(long.normal_str.is_none());
        assert_eq!(
            long.flags,
            Some(Vec::from(["help", "foo"].map(String::from)))
        );

        let short = parse_args(cli(), args(&["-h"])).unwrap();
        assert_eq!(short.flags, Some(vec!["h".to_string()]));
        assert!(short.value_flags.is_none());
        assert!(short.normal_str.is_none());
    }

    #[test]
    fn character_mixed() {
        let parsed = parse_args(cli(), args(&["-abc", "foo"])).unwrap();
        assert!(parsed.normal_str.is_none());
        assert_eq!(parsed.flags, Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(
            parsed.value_flags,
            Some(vec![("c".to_string(), "foo".to_string())])
        )
    }

    #[test]
    fn value_flag_with_value_separated() {
        let key = "secret";
        let parsed = parse_args(cli(), args(&["--bar", key, "--bat", key])).unwrap();
        assert_eq!(
            parsed.value_flags,
            Some(vec![
                ("bar".to_string(), key.to_string()),
                ("bat".to_string(), key.to_string())
            ])
        );
        assert!(parsed.normal_str.is_none());
        assert!(parsed.flags.is_none());
    }

    #[test]
    fn value_flag_with_equal_sign() {
        let parsed = parse_args(cli(), args(&["--bar=secret"])).unwrap();
        assert_eq!(
            parsed.value_flags,
            Some(vec![("bar".to_string(), "secret".to_string())])
        );
        assert!(parsed.normal_str.is_none());
        assert!(parsed.flags.is_none());
    }

    #[test]
    fn valued_flag_without_value_is_an_error() {
        assert!(parse_args(cli(), args(&["--bar"])).is_err());
    }

    #[test]
    fn unknown_flag_is_an_error() {
        assert!(parse_args(cli(), args(&["--nope"])).is_err());
    }

    #[test]
    fn double_dash_stops_flag_parsing() {
        let parsed = parse_args(cli(), args(&["--", "--help", "--bar"])).unwrap();
        assert!(parsed.flags.is_none());
        assert!(parsed.value_flags.is_none());
        assert_eq!(
            parsed.normal_str,
            Some(Vec::from(["--help", "--bar"].map(String::from)))
        );
    }

    #[test]
    fn optional_flag_takes_the_next_value_when_present() {
        let parsed = parse_args(cli(), args(&["--baz", "1"])).unwrap();
        assert!(parsed.flags.is_none());
        assert!(parsed.normal_str.is_none());
        assert_eq!(
            parsed.value_flags,
            Some(vec![("baz".to_string(), "1".to_string())])
        );
    }

    #[test]
    fn optional_flag_works_as_a_boolean_when_last() {
        let parsed = parse_args(cli(), args(&["--baz"])).unwrap();
        assert!(parsed.normal_str.is_none());
        assert!(parsed.value_flags.is_none());
        assert_eq!(parsed.flags, Some(vec!["baz".to_string()]));
    }

    #[test]
    fn optional_flag_with_equal_sign() {
        let parsed = parse_args(cli(), args(&["--baz=foo"])).unwrap();
        assert!(parsed.flags.is_none());
        assert!(parsed.normal_str.is_none());
        assert_eq!(
            parsed.value_flags,
            Some(vec![("baz".to_string(), "foo".to_string())])
        );
    }

    #[test]
    fn empty_flagset_accepts_any_flag() {
        let parsed = parse_args(Flagset::new_empty(), args(&["--whatever"])).unwrap();
        assert_eq!(parsed.flags, Some(vec!["whatever".to_string()]));
    }

    #[test]
    fn everything_mixed() {
        let parsed = parse_args(
            cli(),
            args(&["-abc=foo", "--foo", "--bar", "asdf", "--baz", "-h", "hello"]),
        )
        .unwrap();
        assert_eq!(
            parsed.flags,
            Some(Vec::from(["a", "b", "foo", "baz", "h"].map(String::from)))
        );
        assert_eq!(
            parsed.value_flags,
            Some(vec![
                ("c".to_string(), "foo".to_string()),
                ("bar".to_string(), "asdf".to_string())
            ])
        );
        assert_eq!(parsed.normal_str, Some(vec!["hello".to_string()]))
    }
}
