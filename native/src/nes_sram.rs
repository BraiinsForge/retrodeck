//! Legacy InfoNES save codec: tag-escaped run-length encoding. Only the
//! decoder is needed at runtime to migrate old saves into raw FCEUmm SRAM.

pub fn maximum_encoded_size(input_size: usize) -> usize {
    input_size * 3 + 1
}

pub fn decode(input: &[u8], output: &mut [u8]) -> bool {
    if input.is_empty() || output.is_empty() {
        return false;
    }
    let mut decoded = vec![0; output.len()];
    let tag = input[0];
    let mut input_index = 1;
    let mut output_index = 0;
    while output_index < output.len() {
        let Some(&value) = input.get(input_index) else {
            return false;
        };
        input_index += 1;
        if value != tag {
            decoded[output_index] = value;
            output_index += 1;
            continue;
        }
        if input.len() - input_index < 2 {
            return false;
        }
        let repeated = input[input_index];
        let run_length = usize::from(input[input_index + 1]) + 1;
        input_index += 2;
        if output.len() - output_index < run_length {
            return false;
        }
        decoded[output_index..output_index + run_length].fill(repeated);
        output_index += run_length;
    }
    output.copy_from_slice(&decoded);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_literal_and_tag_escaped_runs() {
        let mut output = [0; 8];
        assert!(decode(&[7, 1, 2, 7, 7, 1, 7, 42, 3], &mut output));
        assert_eq!(output, [1, 2, 7, 7, 42, 42, 42, 42]);
    }

    #[test]
    fn rejects_truncated_and_oversized_payloads() {
        let mut output = [0xA5; 8];
        for input in [&[][..], &[7, 7], &[7, 7, 1, 200], &[0xFF, 0x42]] {
            assert!(!decode(input, &mut output));
            assert_eq!(output, [0xA5; 8]);
        }
    }
}
