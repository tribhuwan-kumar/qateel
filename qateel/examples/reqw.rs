use reqwest::header::{ACCEPT_RANGES, RANGE};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "https://video-downloads.googleusercontent.com/ADGPM2nqxD0s4sHKtNQYhZEtCFPPWzzb8cb9P_uPloHLC4vYUjh5T1174iL68Ts8ojFKLZsu8LOX7us-Kgq5JO0FNUjsV4yQIsqdHKEOpfvT2PhDXWPmvw64SP9QQjkXAyLwZ_0zIpJij6K0dHwEAaM-lGzdO8Aj4Thsdq7oo9Bw4pxLGs4gkFUwzbw2OTBlYaKO3yAb5wq2vG2mWAIAHh19SbgWvFWQg5A1mH-mvRKANocmtTk_PuGmUdQfT63Pb1sM6J7x57KP6sc-mpAY6HUT3cclGzFgB-zxn_Bx01OJpgc7CzWLBqyFMMVFm2cFlGDLTJx0g1ApdcQyhS9W9t3D8JPog53KOSpJePdjq-t8fvK15SpFWCQr_mL85DNJIalCpqrnD7_gh3dBL6cqHsxFrhazrc86rT5Jg6jNeK0EkLTDNC5U3Yb0-AwLPomNEiTh9S-MZwcAiYxavf1gQ1_1ZFeuFYzdunFOyLCFniXs4exj689xZ49IYfLL_LuRTr1wny6Px1IgUS9NXeU026CHPD72d_mFDILx_Nrq8LD3KsM4BGhlA4YA0wFYJ1stsFtA3wub56ERu3dD0qS0YT-dtAwwysUkPpVCZD5T-ouW82SAOtY7sHGhJ_TiMsHkuQ9QK0g6dnpCZZ7s_sfCDwuok5lrXV2TEMd8Yyze4OpQPfgLcLfaZnL0gvFuU_X9au2LLP5F_pldL_nZ5-1vPnfOXVB-aT9M7yQHoLu7YvSjZXDzWjpg5VcFXAipnRcbCfUTwEuOvYHnMN_l_SiADVKlQ1xKlAIxhxwF0OyCx5DXcHPY5neCaHNcYmGRRHIumsiwt16k-4DSvQ61iokzcgX--euyh-euhL5XyebiIwtq7j1fysU-EZK3qjKMahVaPhkYOPcDqZYuTzmThkyjs2DwIa_ucCHjZExJDlMA_Ql6nPnDKJ3Q32PV1ejTkehl0HzyS0chHjD3WY1J-85qyiW2qhP3cfkIHSiCAOXdS7IVhBnw8MZWFNjPrc34itb4qUBWNS3hUHR4uHZ5SBXN0qZlUE-Uc8EDZhYwfYZer8QFfmlU4XCz1T9JRPSNDqBFhGOev7eOiJbH";
    let client = reqwest::Client::new();

    let res = client.head(url).send().await?;

    let supports_resume = if let Some(val) = res.headers().get(ACCEPT_RANGES) {
        val == "bytes"
    } else {
        let probe = client.get(url)
            .header(RANGE, "bytes=0-0")
            .send()
            .await?;
        probe.status() == reqwest::StatusCode::PARTIAL_CONTENT
    };

    if supports_resume {
        println!("Resume is supported!");
    } else {
        println!("Resume is NOT supported.");
    }

    Ok(())
}
