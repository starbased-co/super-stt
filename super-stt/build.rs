fn main() {
    // Print the CUDA compute capability during build
    if let Ok(cuda_cap) = std::env::var("CUDA_COMPUTE_CAP") {
        println!("cargo:warning=Building with CUDA_COMPUTE_CAP={cuda_cap}");
        println!("cargo:rustc-env=CUDA_COMPUTE_CAP={cuda_cap}");

        // Validate the compute capability value
        match cuda_cap.as_str() {
            "75" => println!("cargo:warning=Targeting SM 7.5 (RTX 2080, T4, Quadro RTX)"),
            "80" => println!("cargo:warning=Targeting SM 8.0 (A100, A30)"),
            "86" => {
                println!("cargo:warning=Targeting SM 8.6 (RTX 3060-3090, A40, RTX A2000-A6000)");
            }

            "89" => {
                println!("cargo:warning=Targeting SM 8.9 (RTX 4050-4090, RTX Ada series, L4, L40)");
            }
            "90" => println!("cargo:warning=Targeting SM 9.0 (H100, H200, GH200)"),
            "100" => println!("cargo:warning=Targeting SM 10.0 (B200, GB200)"),
            "120" => println!("cargo:warning=Targeting SM 12.0 (RTX 5050-5090, RTX PRO Blackwell)"),
            other => println!("cargo:warning=Unknown CUDA compute capability: {other}"),
        }
    } else {
        println!("cargo:warning=CUDA_COMPUTE_CAP not set - building for generic CUDA or CPU");
    }

    // Check features
    if cfg!(feature = "cuda") {
        println!("cargo:warning=CUDA feature is enabled");
    }
    if cfg!(feature = "cudnn") {
        println!("cargo:warning=cuDNN feature is enabled");
    }

    // For cross-compilation verification
    if let Ok(target) = std::env::var("TARGET") {
        println!("cargo:warning=Building for target: {target}");
    }
}
