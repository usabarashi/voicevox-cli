#[cfg(test)]
mod tests {
    use std::time::Duration;
    use voicevox_cli::paths::get_socket_path;

    #[test]
    fn test_exponential_backoff_timing() {
        // Test that exponential backoff follows expected pattern
        let delays = [100, 200, 400, 800, 1000, 1000, 1000, 1000, 1000];
        let mut total_delay = Duration::from_millis(0);

        for (i, &expected_ms) in delays.iter().enumerate() {
            let delay = Duration::from_millis(100) * 2_u32.pow(i as u32).min(10);
            let capped_delay = delay.min(Duration::from_secs(1));
            assert_eq!(capped_delay.as_millis(), expected_ms);
            total_delay += capped_delay;
        }

        // Total should be 6.5 seconds
        assert_eq!(total_delay.as_millis(), 6500);
    }

    #[test]
    fn test_socket_path_consistency() {
        // Ensure socket path is consistent across calls
        let path1 = get_socket_path();
        let path2 = get_socket_path();
        assert_eq!(path1, path2);

        // Check that path is reasonable
        assert!(path1.to_string_lossy().contains("voicevox"));
    }
}
