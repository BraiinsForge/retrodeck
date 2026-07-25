//! Legacy InfoNES save codec: tag-escaped run-length encoding. Only the
//! decoder is needed at runtime to migrate old saves into raw FCEUmm SRAM.

pub fn maximum_encoded_size(input_size: usize) -> usize {
    input_size * 3 + 1
}

pub fn decode(input: &[u8], output: &mut [u8]) -> bool {
    if input.is_empty() || output.is_empty() {
        return false;
    }
    let tag = input[0];
    let mut input_index = 1;
    let mut output_index = 0;
    while output_index < output.len() {
        let Some(&value) = input.get(input_index) else {
            return false;
        };
        input_index += 1;
        if value != tag {
            output[output_index] = value;
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
        output[output_index..output_index + run_length].fill(repeated);
        output_index += run_length;
    }
    true
}

#[cfg(test)]
pub fn encode(input: &[u8], output: &mut Vec<u8>) -> bool {
    if input.is_empty() {
        return false;
    }
    let mut frequency = [0_usize; 256];
    for &value in input {
        frequency[usize::from(value)] += 1;
    }
    let mut tag = 0_u8;
    for value in 1..=255_u8 {
        if frequency[usize::from(value)] < frequency[usize::from(tag)] {
            tag = value;
        }
    }
    output.clear();
    output.push(tag);
    let mut input_index = 0;
    while input_index < input.len() {
        let value = input[input_index];
        let mut run_length = 1;
        while input_index + run_length < input.len()
            && run_length < 256
            && input[input_index + run_length] == value
        {
            run_length += 1;
        }
        if run_length >= 4 || value == tag {
            output.push(tag);
            output.push(value);
            output.push((run_length - 1) as u8);
        } else {
            output.extend(std::iter::repeat_n(value, run_length));
        }
        input_index += run_length;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_the_cpp_run_length_format() {
        let sram: Vec<u8> = (0..8192_u32)
            .map(|index| if index % 97 == 0 { 0xAA } else { (index % 7) as u8 })
            .collect();
        let mut encoded = Vec::new();
        assert!(encode(&sram, &mut encoded));
        assert!(encoded.len() <= maximum_encoded_size(sram.len()));
        let mut decoded = vec![0_u8; sram.len()];
        assert!(decode(&encoded, &mut decoded));
        assert_eq!(decoded, sram);
    }

    #[test]
    fn rejects_truncated_and_oversized_payloads() {
        let mut output = [0_u8; 8];
        assert!(!decode(&[], &mut output));
        assert!(!decode(&[7, 7], &mut output));
        assert!(!decode(&[7, 7, 1, 200], &mut output));
    }
}
