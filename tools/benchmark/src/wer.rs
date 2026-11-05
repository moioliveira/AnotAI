//! Word Error Rate: distância de edição entre palavras / palavras da referência.

/// Normaliza para comparar: minúsculas, sem pontuação, espaços únicos.
/// Mantém acentos (em pt-BR, "e"/"é" e "a"/"à" são erros de verdade).
pub fn normalize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '-'))
        .map(|w| w.trim_matches(|c| c == '\'' || c == '-'))
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Substituições + inserções + remoções (Levenshtein por palavra).
pub fn edit_distance(reference: &[String], hypothesis: &[String]) -> usize {
    let mut prev: Vec<usize> = (0..=hypothesis.len()).collect();
    let mut cur = vec![0; hypothesis.len() + 1];
    for (i, r) in reference.iter().enumerate() {
        cur[0] = i + 1;
        for (j, h) in hypothesis.iter().enumerate() {
            let substitution = prev[j] + usize::from(r != h);
            cur[j + 1] = substitution.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[hypothesis.len()]
}

/// (erros, palavras da referência).
pub fn wer_counts(reference: &str, hypothesis: &str) -> (usize, usize) {
    let r = normalize(reference);
    let h = normalize(hypothesis);
    (edit_distance(&r, &h), r.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_text_has_no_errors() {
        assert_eq!(wer_counts("Bom dia, tudo bem?", "bom dia tudo bem"), (0, 4));
    }

    #[test]
    fn counts_substitution_insertion_deletion() {
        // "quinta-feira" → "quem está feira": 1 substituição + 2 inserções.
        assert_eq!(
            wer_counts("marcada para quinta-feira", "marcada para quem está feira"),
            (3, 3)
        );
        assert_eq!(wer_counts("a b c", "a c"), (1, 3));
        assert_eq!(wer_counts("a b", "a b c"), (1, 2));
    }

    #[test]
    fn accents_matter() {
        assert_eq!(wer_counts("ontem à tarde", "ontem a tarde"), (1, 3));
    }

    #[test]
    fn empty_cases() {
        assert_eq!(wer_counts("", ""), (0, 0));
        assert_eq!(wer_counts("um dois", ""), (2, 2));
    }
}
