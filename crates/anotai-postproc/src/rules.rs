//! Regras determinísticas aplicadas antes do LLM (rápidas e offline):
//! comandos de voz, hesitações, dicionário pessoal, espaçamento e maiúsculas.

use std::sync::LazyLock;

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

/// Termo do dicionário pessoal (tabela `dictionary_term`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Term {
    /// Grafia correta, ex.: "eSales".
    pub term: String,
    /// Como o STT costuma ouvir, ex.: "i sales". Vazio = só acerta maiúsculas.
    pub spoken_as: Option<String>,
}

fn re(pattern: &str) -> Regex {
    // Padrões fixos deste arquivo, cobertos pelos testes.
    #[allow(clippy::expect_used)]
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .expect("regex fixa inválida")
}

/// (padrão, substituição). Frases mais longas primeiro. A pontuação que o STT
/// às vezes coloca em volta da palavra falada é absorvida.
static VOICE_COMMANDS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    let around = |words: &str| format!(r"[,.;:]?\s*\b(?:{words})\b[,.;:]?");
    vec![
        (re(&around("novo parágrafo")), "\n\n"),
        (re(&around("nova linha|próxima linha")), "\n"),
        (re(&around("ponto de interrogação")), "?"),
        (re(&around("ponto de exclamação")), "!"),
        (re(&around("ponto e vírgula")), ";"),
        (re(&around("ponto final")), "."),
        (re(&around("dois pontos")), ":"),
        (re(&around("vírgula")), ","),
        (re(r"\s*\babre parênteses?\b[,.]?\s*"), " ("),
        (re(r"[,.]?\s*\bfecha parênteses?\b"), ")"),
    ]
});

/// Hesitações sem significado. "é" sozinho é verbo: só "éé" em diante sai.
static FILLERS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        (
            re(r"(?:^|\s)(?:é{2,}h*|eh+|hã+|ãh+|hu+m+|hm+|ahn+|uhm+)\b[,.]?"),
            "",
        ),
        // "tipo assim" e "tipo," como muleta.
        (re(r"\btipo assim\b,?"), ""),
        (re(r"\btipo,"), ""),
        // "né" como marcador no fim de frase: "..., né?" → "...?"
        // (o crate regex não tem lookahead: a pontuação é capturada e devolvida).
        (re(r",?\s*\bné\b(\s*(?:[?.!,;]|$))"), "$1"),
    ]
});

static SPACE_BEFORE_PUNCT: LazyLock<Regex> = LazyLock::new(|| re(r"[ \t]+([,.;:!?)])"));
static SPACE_AFTER_OPEN: LazyLock<Regex> = LazyLock::new(|| re(r"\(\s+"));
static MISSING_SPACE: LazyLock<Regex> = LazyLock::new(|| re(r"([,;:!?])(\p{L})"));
static DUP_PUNCT: LazyLock<Regex> = LazyLock::new(|| re(r"([,;:])[ \t]*[,;:]+"));
static COMMA_BEFORE_END: LazyLock<Regex> = LazyLock::new(|| re(r"[,;:]+([.!?])"));
static MULTI_SPACE: LazyLock<Regex> = LazyLock::new(|| re(r"[ \t]{2,}"));
static SPACE_AROUND_NL: LazyLock<Regex> = LazyLock::new(|| re(r"[ \t]*\n[ \t]*"));
static LEADING_PUNCT: LazyLock<Regex> = LazyLock::new(|| re(r"^[\s,;:]+"));

/// Troca comandos falados por pontuação.
pub fn apply_voice_commands(text: &str) -> String {
    VOICE_COMMANDS
        .iter()
        .fold(text.to_owned(), |acc, (re, rep)| {
            re.replace_all(&acc, *rep).into_owned()
        })
}

/// Remove hesitações.
pub fn remove_fillers(text: &str) -> String {
    FILLERS.iter().fold(text.to_owned(), |acc, (re, rep)| {
        re.replace_all(&acc, *rep).into_owned()
    })
}

/// Aplica o dicionário pessoal.
pub fn apply_dictionary(text: &str, terms: &[Term]) -> String {
    let mut out = text.to_owned();
    for t in terms {
        let source = t
            .spoken_as
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&t.term);
        let Ok(pattern) = RegexBuilder::new(&format!(r"\b{}\b", regex::escape(source.trim())))
            .case_insensitive(true)
            .build()
        else {
            continue;
        };
        out = pattern
            .replace_all(&out, regex::NoExpand(&t.term))
            .into_owned();
    }
    out
}

/// Arruma espaços em volta da pontuação e as maiúsculas de início de frase.
pub fn tidy(text: &str) -> String {
    let mut s = SPACE_AROUND_NL.replace_all(text, "\n").into_owned();
    s = DUP_PUNCT.replace_all(&s, "$1").into_owned();
    s = COMMA_BEFORE_END.replace_all(&s, "$1").into_owned();
    s = SPACE_BEFORE_PUNCT.replace_all(&s, "$1").into_owned();
    s = SPACE_AFTER_OPEN.replace_all(&s, "(").into_owned();
    s = MISSING_SPACE.replace_all(&s, "$1 $2").into_owned();
    s = MULTI_SPACE.replace_all(&s, " ").into_owned();
    s = LEADING_PUNCT.replace_all(&s, "").into_owned();
    capitalize_sentences(s.trim_end_matches([' ', '\t']).trim_start())
}

fn capitalize_sentences(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut capitalize = true;
    for c in text.chars() {
        if capitalize && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            capitalize = false;
            continue;
        }
        if matches!(c, '.' | '!' | '?' | '\n') {
            capitalize = true;
        } else if !c.is_whitespace() && !matches!(c, '"' | '(' | '\'' | '“') {
            capitalize = false;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(text: &str) -> String {
        tidy(&remove_fillers(&apply_voice_commands(text)))
    }

    #[test]
    fn spoken_punctuation_becomes_symbols() {
        assert_eq!(
            all("olá vírgula tudo bem ponto de interrogação"),
            "Olá, tudo bem?"
        );
        assert_eq!(
            all("primeiro item dois pontos maçã ponto final nova linha segundo"),
            "Primeiro item: maçã.\nSegundo"
        );
    }

    #[test]
    fn absorbs_punctuation_the_stt_already_added() {
        assert_eq!(all("Olá, vírgula, tudo bem."), "Olá, tudo bem.");
        assert_eq!(all("Fim. Ponto final."), "Fim.");
    }

    #[test]
    fn paragraphs_and_parentheses() {
        assert_eq!(
            all("veja abre parênteses página dois fecha parênteses novo parágrafo depois"),
            "Veja (página dois)\n\nDepois"
        );
    }

    #[test]
    fn removes_hesitations_but_keeps_verb_e() {
        assert_eq!(
            all("éé o relatório é hã importante"),
            "O relatório é importante"
        );
        assert_eq!(all("ele vem amanhã, né?"), "Ele vem amanhã?");
        assert_eq!(all("tipo assim, eu acho que sim"), "Eu acho que sim");
        assert_eq!(all("é isso"), "É isso");
    }

    #[test]
    fn dictionary_replaces_spoken_form_and_fixes_case() {
        let terms = [
            Term {
                term: "eSales".into(),
                spoken_as: Some("i sales".into()),
            },
            Term {
                term: "AnotAI".into(),
                spoken_as: None,
            },
        ];
        assert_eq!(
            apply_dictionary("a I Sales usa o anotai", &terms),
            "a eSales usa o AnotAI"
        );
    }

    #[test]
    fn dictionary_term_with_regex_chars_is_literal() {
        let terms = [Term {
            term: "C++".into(),
            spoken_as: Some("c mais mais".into()),
        }];
        assert_eq!(
            apply_dictionary("código em c mais mais", &terms),
            "código em C++"
        );
    }

    #[test]
    fn tidy_capitalizes_after_sentence_end() {
        assert_eq!(tidy("oi. tudo bem? sim"), "Oi. Tudo bem? Sim");
        assert_eq!(tidy("ok ,certo"), "Ok, certo");
    }
}
