use std::num::Wrapping;

// Constante mágica do algoritmo MurmurHash2 modificado usado pela CurseForge
// O Wrapping garante que o comportamento de overflow seja idêntico ao Go/C++
const MULTIPLEX: Wrapping<u32> = Wrapping(1540483477);

// Caracteres ignorados pelo algoritmo da CurseForge (whitespace)
#[inline(always)]
fn is_ignored_in_curseforge_fingerprint(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | b'\r' | b' ')
}

fn compute_curseforge_fingerprint_normalized_length(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .filter(|&&b| !is_ignored_in_curseforge_fingerprint(b))
        .count()
}

pub fn calculate_fingerprint(bytes: &[u8]) -> String {
    let len_no_whitespace = compute_curseforge_fingerprint_normalized_length(bytes);

    let mut num1 = Wrapping(len_no_whitespace as u32);
    let mut num2 = Wrapping(1u32) ^ num1;
    let mut num3 = Wrapping(0u32);
    let mut num4 = 0u32;

    for &b in bytes {
        if !is_ignored_in_curseforge_fingerprint(b) {
            // Lógica bitwise portada do Go: num3 |= uint32(b) << num4
            num3 = num3 | (Wrapping(b as u32) << num4 as usize);
            num4 += 8;

            if num4 == 32 {
                let num6 = num3 * MULTIPLEX;
                // num7 := (num6 ^ num6>>24) * multiplex
                let num7 = (num6 ^ (num6 >> 24)) * MULTIPLEX;

                // num2 = num2*multiplex ^ num7
                num2 = (num2 * MULTIPLEX) ^ num7;

                num3 = Wrapping(0);
                num4 = 0;
            }
        }
    }

    if num4 > 0 {
        num2 = (num2 ^ num3) * MULTIPLEX;
    }

    let num6 = (num2 ^ (num2 >> 13)) * MULTIPLEX;

    // Retorna o hash como String numérica (ex: "12345678")
    let result = num6 ^ (num6 >> 15);
    result.0.to_string()
}