// SPDX-License-Identifier: GPL-3.0-only
#[cfg(test)]
mod integration_tests {
    use crate::auth::UdpAuth;
    use std::env;
    use std::sync::Mutex;

    // Ensure tests run sequentially to avoid race conditions with environment variables
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_client_daemon_auth_flow() {
        let _guard = TEST_MUTEX.lock().unwrap();
        use std::time::{SystemTime, UNIX_EPOCH};

        // Set up a unique temporary directory for testing
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_id = std::thread::current().id();
        let temp_dir = env::temp_dir().join(format!("super_stt_auth_test_{timestamp}_{test_id:?}"));

        // Store original value to restore later
        let original_runtime_dir = env::var("XDG_RUNTIME_DIR").ok();
        unsafe {
            env::set_var("XDG_RUNTIME_DIR", &temp_dir);
        }

        // Simulate daemon starting and creating auth
        let daemon_auth = UdpAuth::new().unwrap();
        // Ensure daemon creates the secret first
        let _daemon_secret = daemon_auth.get_or_create_secret().unwrap();

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

        // Restore original environment variable
        unsafe {
            match original_runtime_dir {
                Some(original) => env::set_var("XDG_RUNTIME_DIR", original),
                None => env::remove_var("XDG_RUNTIME_DIR"),
            }
        }
    }

    #[test]
    fn test_auth_persistence() {
        let _guard = TEST_MUTEX.lock().unwrap();
        use std::time::{SystemTime, UNIX_EPOCH};

        // Set up a unique temporary directory for testing with timestamp and thread ID
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let thread_id = std::thread::current().id();
        let temp_dir = env::temp_dir().join(format!(
            "super_stt_persistence_test_{timestamp}_{thread_id:?}"
        ));

        // Store original value to restore later
        let original_runtime_dir = env::var("XDG_RUNTIME_DIR").ok();
        unsafe {
            env::set_var("XDG_RUNTIME_DIR", &temp_dir);
        }

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

        // Restore original environment variable
        unsafe {
            match original_runtime_dir {
                Some(original) => env::set_var("XDG_RUNTIME_DIR", original),
                None => env::remove_var("XDG_RUNTIME_DIR"),
            }
        }
    }
}
