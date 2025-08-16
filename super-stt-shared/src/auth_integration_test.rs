// SPDX-License-Identifier: GPL-3.0-only
#[cfg(test)]
mod integration_tests {
    use crate::auth::UdpAuth;
    use std::env;

    #[test]
    fn test_client_daemon_auth_flow() {
        // Set up a unique temporary directory for testing
        let test_id = std::thread::current().id();
        let temp_dir = env::temp_dir().join(format!("super_stt_auth_test_{test_id:?}"));
        unsafe {
            env::set_var("XDG_RUNTIME_DIR", &temp_dir);
        }

        // Simulate daemon starting and creating auth
        let daemon_auth = UdpAuth::new().unwrap();

        // Simulate client (applet) connecting
        let client_auth = UdpAuth::new().unwrap();
        let client_message = client_auth.create_auth_message("applet").unwrap();

        // Simulate daemon verifying client
        let verified_client = daemon_auth.verify_auth_message(&client_message).unwrap();
        assert_eq!(verified_client, Some("applet".to_string()));

        // Simulate invalid client
        let invalid_message = "REGISTER:applet:wrong_secret";
        let invalid_result = daemon_auth.verify_auth_message(invalid_message).unwrap();
        assert_eq!(invalid_result, None);

        // Test different client types
        let app_message = client_auth.create_auth_message("app").unwrap();
        let verified_app = daemon_auth.verify_auth_message(&app_message).unwrap();
        assert_eq!(verified_app, Some("app".to_string()));

        // Cleanup
        daemon_auth.cleanup().unwrap();

        // Verify cleanup worked
        assert!(!temp_dir.join("super-stt").join("udp_secret").exists());
    }

    #[test]
    fn test_auth_persistence() {
        use std::time::{SystemTime, UNIX_EPOCH};

        // Set up a unique temporary directory for testing with timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("super_stt_persistence_test_{timestamp}"));
        unsafe {
            env::set_var("XDG_RUNTIME_DIR", &temp_dir);
        }

        // Clean up any existing secret first
        let cleanup_auth = UdpAuth::new().unwrap();
        let _ = cleanup_auth.cleanup();

        // Create auth and generate secret
        let auth1 = UdpAuth::new().unwrap();
        let secret1 = auth1.get_or_create_secret().unwrap();

        // Verify the secret file exists
        assert!(temp_dir.join("super-stt").join("udp_secret").exists());

        // Create second auth instance - should get same secret from file
        let auth2 = UdpAuth::new().unwrap();
        let secret2 = auth2.get_or_create_secret().unwrap();

        assert_eq!(secret1, secret2);

        // Messages should be compatible
        let message1 = auth1.create_auth_message("test").unwrap();
        let verified = auth2.verify_auth_message(&message1).unwrap();
        assert_eq!(verified, Some("test".to_string()));

        // Cleanup
        auth1.cleanup().unwrap();
    }
}
