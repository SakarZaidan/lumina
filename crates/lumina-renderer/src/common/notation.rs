//! LaTeX and `MathML` to Unicode, shared by both backends.
//!
//! Neither is typeset. A formula becomes a plain string of Unicode characters
//! — Greek letters, operators, super- and subscripts — drawn by the ordinary
//! text path (ADR-0012). Both backends draw the same string, so it is decided
//! once, here.

/// Convert common LaTeX and math notation to Unicode for plain-text rendering.
///
/// This is the whole of Lumina's LaTeX support: substitution, not typesetting.
/// `mitex` was declared as a dependency for three releases and imported by
/// nothing; ADR-0012 records the decision to drop it and describe this
/// honestly rather than imply an engine that was never wired up.
///
/// Handles the Greek alphabet, common operators, `\frac{a}{b}` as `a/b`,
/// super- and subscripts, `\sqrt`, `\sum`, `\int`, and spacing commands.
/// Anything it does not recognise is passed through rather than dropped.
pub fn latex_to_unicode(expr: &str) -> String {
    let mut s = expr.to_string();

    // Spacing commands → collapse to a single (or no) space.
    s = s
        .replace(r"\,", " ")
        .replace(r"\;", " ")
        .replace(r"\:", " ")
        .replace(r"\!", "")
        .replace(r"\quad", "  ")
        .replace(r"\qquad", "    ");

    // Greek letters
    s = s
        .replace(r"\alpha", "α")
        .replace(r"\beta", "β")
        .replace(r"\gamma", "γ")
        .replace(r"\delta", "δ")
        .replace(r"\epsilon", "ε")
        .replace(r"\zeta", "ζ")
        .replace(r"\eta", "η")
        .replace(r"\theta", "θ")
        .replace(r"\iota", "ι")
        .replace(r"\kappa", "κ")
        .replace(r"\lambda", "λ")
        .replace(r"\mu", "μ")
        .replace(r"\nu", "ν")
        .replace(r"\xi", "ξ")
        .replace(r"\pi", "π")
        .replace(r"\rho", "ρ")
        .replace(r"\sigma", "σ")
        .replace(r"\tau", "τ")
        .replace(r"\phi", "φ")
        .replace(r"\chi", "χ")
        .replace(r"\psi", "ψ")
        .replace(r"\omega", "ω");

    // Operators and symbols
    s = s
        .replace(r"\times", "×")
        .replace(r"\div", "÷")
        .replace(r"\pm", "±")
        .replace(r"\leq", "≤")
        .replace(r"\geq", "≥")
        .replace(r"\neq", "≠")
        .replace(r"\approx", "≈")
        .replace(r"\infty", "∞")
        .replace(r"\sum", "Σ")
        .replace(r"\prod", "Π")
        .replace(r"\int", "∫")
        .replace(r"\sqrt", "√")
        .replace(r"\cdot", "·")
        .replace(r"\circ", "∘")
        .replace(r"\in", "∈")
        .replace(r"\cup", "∪")
        .replace(r"\cap", "∩")
        .replace(r"\subset", "⊂")
        .replace(r"\rightarrow", "→")
        .replace(r"\leftarrow", "←")
        .replace(r"\Rightarrow", "⇒")
        .replace(r"\to", "→")
        .replace(r"\nabla", "∇")
        .replace(r"\partial", "∂")
        .replace(r"\cdots", "⋯")
        .replace(r"\ldots", "…")
        .replace(r"\angle", "∠")
        .replace(r"\vec", "")
        .replace(r"\left", "")
        .replace(r"\right", "");

    // Trig / named functions (LaTeX style)
    s = s
        .replace(r"\sin", "sin")
        .replace(r"\cos", "cos")
        .replace(r"\tan", "tan")
        .replace(r"\log", "log")
        .replace(r"\ln", "ln")
        .replace(r"\exp", "exp")
        .replace(r"\lim", "lim")
        .replace(r"\max", "max")
        .replace(r"\min", "min");

    // Fractions: \frac{a}{b} → a/b (brace-balanced, nestable).
    s = replace_frac(&s);

    // Super/subscripts: ^{...}, _{...}, bare ^N / _N.
    s = replace_scripts(&s);

    // Safety net: strip any remaining `\command` token (while braces still
    // delimit its argument) so unhandled commands never leak as literal text.
    s = strip_leftover_commands(&s);

    // Remove remaining LaTeX braces.
    s = s.replace(['{', '}'], "");

    s
}

/// Remove any remaining backslash command (`\` followed by ASCII letters), and
/// any lone backslash, leaving surrounding text intact.
fn strip_leftover_commands(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 1;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Superscript Unicode for a character, if one exists.
fn superscript_of(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'a' => 'ᵃ',
        'b' => 'ᵇ',
        'c' => 'ᶜ',
        'd' => 'ᵈ',
        'e' => 'ᵉ',
        'f' => 'ᶠ',
        'g' => 'ᵍ',
        'h' => 'ʰ',
        'i' => 'ⁱ',
        'j' => 'ʲ',
        'k' => 'ᵏ',
        'l' => 'ˡ',
        'm' => 'ᵐ',
        'n' => 'ⁿ',
        'o' => 'ᵒ',
        'p' => 'ᵖ',
        'r' => 'ʳ',
        's' => 'ˢ',
        't' => 'ᵗ',
        'u' => 'ᵘ',
        'v' => 'ᵛ',
        'w' => 'ʷ',
        'x' => 'ˣ',
        'y' => 'ʸ',
        'z' => 'ᶻ',
        _ => return None,
    })
}

/// Subscript Unicode for a character, if one exists.
fn subscript_of(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'a' => 'ₐ',
        'e' => 'ₑ',
        'h' => 'ₕ',
        'i' => 'ᵢ',
        'j' => 'ⱼ',
        'k' => 'ₖ',
        'l' => 'ₗ',
        'm' => 'ₘ',
        'n' => 'ₙ',
        'o' => 'ₒ',
        'p' => 'ₚ',
        'r' => 'ᵣ',
        's' => 'ₛ',
        't' => 'ₜ',
        'u' => 'ᵤ',
        'v' => 'ᵥ',
        'x' => 'ₓ',
        _ => return None,
    })
}

/// Replace every `\frac{num}{den}` with `num/den`, honoring nested braces.
fn replace_frac(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i..].starts_with(&['\\', 'f', 'r', 'a', 'c']) {
            let after = i + 5;
            if let Some((num, j)) = read_brace_group(&chars, after) {
                if let Some((den, k)) = read_brace_group(&chars, j) {
                    // Recurse so nested fractions resolve too.
                    let num_s = replace_frac(&num.iter().collect::<String>());
                    let den_s = replace_frac(&den.iter().collect::<String>());
                    let wrap = |t: &str| {
                        if t.chars().count() > 1 && t.contains(['+', '-', ' ', '/']) {
                            format!("({t})")
                        } else {
                            t.to_string()
                        }
                    };
                    out.push_str(&wrap(&num_s));
                    out.push('/');
                    out.push_str(&wrap(&den_s));
                    i = k;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// If `chars[start]` is `{`, return the balanced group contents and the index
/// just past the closing `}`.
fn read_brace_group(chars: &[char], start: usize) -> Option<(Vec<char>, usize)> {
    if start >= chars.len() || chars[start] != '{' {
        return None;
    }
    let mut depth = 0;
    let mut content = Vec::new();
    let mut i = start;
    while i < chars.len() {
        match chars[i] {
            '{' => {
                if depth > 0 {
                    content.push('{');
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((content, i + 1));
                }
                content.push('}');
            }
            c => content.push(c),
        }
        i += 1;
    }
    None
}

/// Convert `^{...}` / `_{...}` and bare `^c` / `_c` runs to Unicode super/subscripts.
/// Characters without a Unicode form keep the `^`/`_` marker so meaning isn't lost.
fn replace_scripts(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let marker = chars[i];
        if marker == '^' || marker == '_' {
            let map = if marker == '^' {
                superscript_of
            } else {
                subscript_of
            };
            i += 1;
            // Collect the script content: a brace group or a single char.
            let content: Vec<char> = if i < chars.len() && chars[i] == '{' {
                match read_brace_group(&chars, i) {
                    Some((c, j)) => {
                        i = j;
                        c
                    }
                    None => vec![],
                }
            } else if i < chars.len() {
                let c = chars[i];
                i += 1;
                vec![c]
            } else {
                vec![]
            };
            // Map each char; if any has no script form, fall back to marker+raw.
            // Collecting into Option<Vec<_>> maps each character exactly once
            // and short-circuits on the first failure, so there is nothing left
            // to unwrap.
            match content
                .iter()
                .map(|&c| map(c))
                .collect::<Option<Vec<char>>>()
            {
                Some(mapped) => out.extend(mapped),
                None => {
                    out.push(marker);
                    out.extend(content.iter());
                }
            }
        } else {
            out.push(marker);
            i += 1;
        }
    }
    out
}

/// Strip `MathML` tags and decode common entities into a plain Unicode string,
/// reusing the same text pipeline as LaTeX.
pub(crate) fn mathml_to_unicode(markup: &str) -> String {
    let mut out = String::with_capacity(markup.len());
    let mut in_tag = false;
    for c in markup.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out = out
        .replace("&times;", "×")
        .replace("&divide;", "÷")
        .replace("&pm;", "±")
        .replace("&pi;", "π")
        .replace("&theta;", "θ")
        .replace("&alpha;", "α")
        .replace("&beta;", "β")
        .replace("&gamma;", "γ")
        .replace("&infin;", "∞")
        .replace("&le;", "≤")
        .replace("&ge;", "≥")
        .replace("&ne;", "≠")
        .replace("&sum;", "Σ")
        .replace("&int;", "∫")
        .replace("&radic;", "√")
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
