use anyhow::{Context, Result, ensure};
use binary_layout::Field;
use binrw::{BinRead, NullString, binrw, helpers::until_eof};
use cryfs_config::config::{CryConfig, ciphers::lookup_cipher_dyn};
use cryfs_crypto::{
    kdf::{KDFParameters, PasswordBasedKDF, scrypt::backends::scrypt::ScryptScrypt},
    symmetric::{Cipher, CipherDef, EncryptionKey},
};
use cryfs_utils::data::Data;
use std::io::{Cursor, Read, Seek};
use thiserror::Error;

const INNER_HEADER: &str = "cryfs.config.inner;0";

#[binrw]
#[brw(little)]
struct InnerConfigLayout {
    header: NullString,
    cipher_name: NullString,
    #[br(parse_with = until_eof)]
    encrypted_config: Vec<u8>,
    // TODO Actually storing these Vecs in an `InnerConfigLayout` object means we have to allocate them during (de)serialization. This could be avoided, maybe by using a `Serializer/Deserializer` system as C++ CryFS cpp-utils had it
}

struct InnerConfig {
    cipher_name: String,
    encrypted_config: Vec<u8>,
}

impl InnerConfig {
    fn deserialize(source: &mut (impl Read + Seek), print_deserialized: bool) -> Result<Self> {
        let layout = InnerConfigLayout::read(source)?;
        let read_header: String = layout
            .header
            .try_into()
            .context("Header is not valid UTF-8")?;
        ensure!(
            read_header == INNER_HEADER,
            "Invalid header in outer config. Expected '{HEADER}', got '{read_header}'",
        );

        if print_deserialized {
            println!(
                "Inner config file header (Null terminated UTF-8 string): \"{}\"",
                INNER_HEADER
            );
        }

        let cipher_name = layout
            .cipher_name
            .try_into()
            .context("Cipher name is not valid UTF-8")?;

        if print_deserialized {
            println!(
                "Inner config file cipher (Null terminated UTF-8 string): \"{}\"",
                cipher_name
            );
        }

        Ok(Self {
            cipher_name,
            encrypted_config: layout.encrypted_config,
        })
    }

    fn decrypt(
        self,
        inner_key: impl FnOnce(usize) -> EncryptionKey,
        print_decrypt: bool,
    ) -> Result<Data> {
        let cipher = lookup_cipher_dyn(&self.cipher_name, inner_key)
            .with_context(|| format!("Trying to look up cipher {}", self.cipher_name))?
            .with_context(|| format!("Trying to look up cipher {}", self.cipher_name))?;

        if print_decrypt {
            println!(
                "Cipher prefix size: {}B",
                cipher.ciphertext_overhead_suffix()
            );
            println!(
                "Cipher suffix size: {}B",
                cipher.ciphertext_overhead_suffix()
            );
        }

        let plaintext = cipher
            .decrypt(self.encrypted_config.into())
            .context("Trying to Cipher::decrypt InnerConfig")?;
        let (plaintext, config_size, padded_size) =
            remove_padding(plaintext).context("Trying to remove padding from InnerConfig")?;

        if print_decrypt {
            println!(
                "Size of unpadded inner config file (stored as 32 byte unsigned integer): {}B ({}B with padding)",
                config_size, padded_size
            )
        }

        Ok(plaintext)
    }
}

binary_layout::binary_layout!(padded_data, LittleEndian, {
    num_bytes: u32,
    data_and_padding: [u8],
});

#[derive(Error, Debug)]
pub enum RemovePaddingError {
    #[error(
        "Padded data claims to store {data_len} bytes but the whole padded blob is only {total_len} bytes."
    )]
    DataTooLarge { data_len: usize, total_len: usize },

    #[error("Padded data claims to store {data_len} bytes, which is larger than usize::MAX.")]
    DataTooLargeForUsize { data_len: u32 },
}

/// Returns the actual config data and how many bytes it takes up without padding.
pub fn remove_padding(mut data: Data) -> Result<(Data, usize, usize), RemovePaddingError> {
    let data_len = padded_data::View::new(&data).num_bytes().read();
    let data_len_usize = usize::try_from(data_len)
        .map_err(|_| RemovePaddingError::DataTooLargeForUsize { data_len })?;
    let total_len = data.len();
    if data.len() < padded_data::data_and_padding::OFFSET + data_len_usize {
        return Err(RemovePaddingError::DataTooLarge {
            data_len: data_len_usize,
            total_len,
        });
    }
    data.shrink_to_subregion(
        padded_data::data_and_padding::OFFSET
            ..(padded_data::data_and_padding::OFFSET + data_len_usize),
    );
    Ok((data, data_len_usize, total_len))
}

fn len(mut source: impl Seek) -> Result<usize> {
    let current_pos = source
        .stream_position()
        .context("Trying to get stream position")?;
    let end_pos = source
        .seek(std::io::SeekFrom::End(0))
        .context("Trying to seek to end of file")?;
    source.seek(std::io::SeekFrom::Start(current_pos))?;
    Ok((end_pos - current_pos) as usize)
}

// Not the
#[binrw]
#[brw(little)]
struct OuterConfigLayout {
    header: NullString,

    kdf_parameters_num_bytes: u64,
    #[br(count = kdf_parameters_num_bytes)]
    kdf_parameters_serialized: Vec<u8>,

    #[br(parse_with = until_eof)]
    encrypted_inner_config: Vec<u8>,
}

struct OuterConfig {
    kdf_parameters_serialized: Vec<u8>,
    encrypted_inner_config: Vec<u8>,
}

const CONFIG_SIZE: usize = 1024;
const HEADER: &str = "cryfs.config;1;scrypt";

impl OuterConfig {
    fn deserialize(source: &mut (impl Read + Seek), print_deserialized: bool) -> Result<Self> {
        let layout =
            OuterConfigLayout::read(source).context("Trying to read config file content")?;
        let config_file_size = len(source).context("Trying to get config file size")?;
        ensure!(
            config_file_size <= CONFIG_SIZE,
            "Config file size {config_file_size} is larger than expected {CONFIG_SIZE}",
        );
        let read_header: String = layout
            .header
            .try_into()
            .context("Header is not valid UTF-8")?;
        ensure!(
            read_header == HEADER,
            "Invalid header in outer config. Expected '{HEADER}', got '{read_header}'",
        );

        if print_deserialized {
            println!(
                "Outer config file header (Null terminated UTF-8 string): \"{}\"",
                HEADER
            );
        }

        assert_eq!(
            layout.kdf_parameters_num_bytes,
            layout.kdf_parameters_serialized.len() as u64
        );

        if print_deserialized {
            println!(
                "Number of bytes for KDF parameters (stored as unsigned 64 byte integer): {}B",
                layout.kdf_parameters_num_bytes
            );
        }

        Ok(Self {
            kdf_parameters_serialized: layout.kdf_parameters_serialized,
            encrypted_inner_config: layout.encrypted_inner_config,
        })
    }

    pub fn decrypt(
        self,
        outer_encryption_key: EncryptionKey,
        print_decrypt: bool,
    ) -> Result<InnerConfig> {
        let cipher = OuterCipher::new(outer_encryption_key)
            .context("Trying to initialize OuterCipher instance")?;

        let plaintext = cipher
            .decrypt(self.encrypted_inner_config.into())
            .context("Trying to Cipher::decrypt OuterConfig")?;

        if print_decrypt {
            println!(
                "Cipher prefix (Nonce) size: {}B",
                cipher.ciphertext_overhead_prefix()
            );
            println!(
                "Cipher suffix (AEAD) size: {}B",
                cipher.ciphertext_overhead_suffix()
            );
        }

        let (plaintext, config_size, padded_size) =
            remove_padding(plaintext).context("Trying to remove padding from OuterConfig")?;

        if print_decrypt {
            println!(
                "Size of unpadded outer config file (stored as 32 byte unsigned integer): {}B ({}B with padding)",
                config_size, padded_size
            )
        }

        let inner_config = InnerConfig::deserialize(&mut Cursor::new(plaintext), print_decrypt)
            .context("Trying to deserialize InnerConfig")?;
        Ok(inner_config)
    }
}

type KDF = ScryptScrypt;
pub type OuterCipher = cryfs_crypto::symmetric::Aes256Gcm;

#[derive(Debug)]
struct ConfigEncryptionKey {
    combined_key: EncryptionKey,
}

impl ConfigEncryptionKey {
    const OUTER_KEY_SIZE: usize = OuterCipher::KEY_SIZE;
    const INNER_MAX_KEY_SIZE: usize = cryfs_crypto::symmetric::MAX_KEY_SIZE;
    const COMBINED_KEY_SIZE: usize = Self::OUTER_KEY_SIZE + Self::INNER_MAX_KEY_SIZE;

    pub fn derive<KDF: PasswordBasedKDF>(
        kdf_parameters: &KDF::Parameters,
        password: &str,
    ) -> ConfigEncryptionKey {
        let combined_key = KDF::derive_key(Self::COMBINED_KEY_SIZE, password, kdf_parameters);

        ConfigEncryptionKey { combined_key }
    }

    pub fn outer_key(&self) -> EncryptionKey {
        self.combined_key.take_bytes(Self::OUTER_KEY_SIZE)
    }

    pub fn inner_key(&self) -> impl Fn(usize) -> EncryptionKey + use<> {
        let inner_key = self.combined_key.skip_bytes(Self::OUTER_KEY_SIZE);
        move |key_num_bytes| inner_key.take_bytes(key_num_bytes)
    }
}

fn derive_key<KDF: PasswordBasedKDF>(
    kdf_parameters_serialized: &[u8],
    password: &str,
) -> Result<ConfigEncryptionKey> {
    let kdf_parameters = KDF::Parameters::deserialize(&kdf_parameters_serialized)
        .context("Trying to deserialize KDF parameters")?;

    Ok(ConfigEncryptionKey::derive::<KDF>(
        &kdf_parameters,
        password,
    ))
}

fn deserialize_into_inner(
    source: &mut (impl Read + Seek),
    password: &str,
    print: bool,
) -> Result<Data> {
    let outer = OuterConfig::deserialize(source, print)?;
    let key = derive_key::<KDF>(&outer.kdf_parameters_serialized, password)?;
    let inner = outer.decrypt(key.outer_key(), print)?;
    inner.decrypt(key.inner_key(), print)
}

pub enum LogMode {
    FullConfig,
    InnerConfig,
}

pub struct ConfigLoader {
    log_mode: Option<LogMode>,
    prettify: bool,
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self {
            log_mode: None,
            prettify: false,
        }
    }

    pub fn with_log_output(log_mode: LogMode, prettify: bool) -> Self {
        Self {
            log_mode: Some(log_mode),
            prettify,
        }
    }

    pub fn load_config(
        &self,
        source: &mut (impl Read + Seek),
        password: &str,
    ) -> Result<CryConfig> {
        let inner_config_data = deserialize_into_inner(
            source,
            password,
            matches!(self.log_mode, Some(LogMode::FullConfig)),
        )?;

        let config = CryConfig::deserialize(&mut Cursor::new(&inner_config_data))?;
        if self.log_mode.is_none() {
            return Ok(config);
        }

        let inner_config_string = if self.prettify {
            let inner_config: serde_json::Value =
                serde_json::from_reader(&mut Cursor::new(inner_config_data))?;
            serde_json::to_string_pretty(&inner_config)?
        } else {
            String::from_utf8_lossy(&inner_config_data).to_string()
        };
        println!("{}", inner_config_string);

        Ok(config)
    }

    pub fn print_config(&self, source: &mut (impl Read + Seek), password: &str) -> Result<()> {
        let inner_config_data = deserialize_into_inner(
            source,
            password,
            matches!(self.log_mode, Some(LogMode::FullConfig)),
        )?;
        let inner_config_string = if self.prettify {
            let inner_config: serde_json::Value =
                serde_json::from_reader(&mut Cursor::new(inner_config_data))?;
            serde_json::to_string_pretty(&inner_config)?
        } else {
            String::from_utf8_lossy(&inner_config_data).to_string()
        };
        println!("{}", inner_config_string);
        Ok(())
    }
}
