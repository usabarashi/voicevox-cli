#[cfg(test)]
mod tests {
    use std::time::Duration;
    use voicevox_cli::infrastructure::daemon::startup;
    use voicevox_cli::infrastructure::paths::get_socket_path;

    #[test]
    fn test_exponential_backoff_timing() {
        // Test that exponential backoff follows expected pattern
        let expected_delays = [100, 200, 400, 800, 1000, 1000, 1000, 1000, 1000];
        let mut total_delay = Duration::from_millis(0);
        let mut delay = startup::initial_retry_delay();

        for &expected_ms in &expected_delays {
            assert_eq!(delay.as_millis(), expected_ms);
            total_delay += delay;
            delay = (delay * 2).min(startup::max_retry_delay());
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
