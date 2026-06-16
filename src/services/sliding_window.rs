/// Non-overlapping token spans for classification (Wolf, NSFW text).
///
/// When `stride == window`, uses [`slice::Chunks`](https://doc.rust-lang.org/std/primitive.slice.html#method.chunks)
/// (fixed-size windows with a possible shorter tail). Overlapping strides use index `step_by`.
pub fn token_windows(token_ids: &[u32], window: usize, stride: usize) -> Vec<&[u32]> {
    if token_ids.is_empty() || window == 0 || stride == 0 {
        return Vec::new();
    }
    if stride == window {
        return token_ids.chunks(window).collect();
    }

    let mut out = Vec::new();
    for start in (0..token_ids.len()).step_by(stride) {
        let end = (start + window).min(token_ids.len());
        out.push(&token_ids[start..end]);
        if end == token_ids.len() {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        assert!(token_windows(&[], 512, 512).is_empty());
    }

    #[test]
    fn single_window_when_short() {
        let ids = vec![1u32, 2, 3];
        let windows = token_windows(&ids, 512, 512);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0], &ids[..]);
    }

    #[test]
    fn four_windows_for_2048() {
        let ids: Vec<u32> = (0..2048).collect();
        let windows = token_windows(&ids, 512, 512);
        assert_eq!(windows.len(), 4);
        assert_eq!(windows[0].len(), 512);
        assert_eq!(windows[3].len(), 512);
    }

    #[test]
    fn tail_window_partial() {
        let ids: Vec<u32> = (0..600).collect();
        let windows = token_windows(&ids, 512, 512);
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].len(), 512);
        assert_eq!(windows[1].len(), 88);
    }

    #[test]
    fn matches_std_chunks_when_stride_equals_window() {
        let ids: Vec<u32> = (0..600).collect();
        let expected: Vec<&[u32]> = ids.chunks(512).collect();
        assert_eq!(token_windows(&ids, 512, 512), expected);
    }
}
