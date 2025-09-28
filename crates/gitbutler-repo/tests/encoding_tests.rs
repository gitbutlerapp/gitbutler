use gitbutler_repo::FileInfo;
use std::path::Path;

#[test]
fn test_latin1_encoding_detection() {
    // Create a Latin-1 encoded byte sequence with non-ASCII characters
    // This represents "café" in Latin-1 encoding
    let latin1_bytes = vec![0x63, 0x61, 0x66, 0xe9]; // "café" in Latin-1

    let path = Path::new("test.txt");
    let file_info = FileInfo::from_content(path, &latin1_bytes);

    // The content should be converted to UTF-8 properly
    assert!(file_info.content.is_some(), "Content should not be None");
    let content = file_info.content.unwrap();
    
    // In Latin-1, 0xe9 is é, which in UTF-8 is 0xc3a9
    // So "café" should be properly decoded
    assert_eq!(content, "café", "Latin-1 content should be properly converted to UTF-8");
    assert_eq!(file_info.mime_type, None, "Should be treated as text file");
}

#[test] 
fn test_utf8_content_unchanged() {
    // Test that valid UTF-8 content remains unchanged
    let utf8_bytes = "café".as_bytes(); // UTF-8 encoded
    
    let path = Path::new("test.txt");
    let file_info = FileInfo::from_content(path, utf8_bytes);
    
    assert!(file_info.content.is_some(), "Content should not be None");
    let content = file_info.content.unwrap();
    assert_eq!(content, "café", "UTF-8 content should remain unchanged");
}

#[test]
fn test_windows1252_encoding() {
    // Windows-1252 has additional characters that Latin-1 doesn't have
    // Test with em-dash (0x97 in Windows-1252)
    let windows1252_bytes = vec![0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x97, 0x77, 0x6f, 0x72, 0x6c, 0x64]; // "Hello—world"
    
    let path = Path::new("test.txt"); 
    let file_info = FileInfo::from_content(path, &windows1252_bytes);
    
    assert!(file_info.content.is_some(), "Content should not be None");
    let content = file_info.content.unwrap();
    
    // Should properly convert to UTF-8
    assert!(content.contains("Hello"), "Should contain Hello");
    assert!(content.contains("world"), "Should contain world");
    // The em-dash (0x97) in Windows-1252 should be converted to UTF-8 em-dash
    assert!(content.len() > 11, "Content should be longer due to UTF-8 encoding of em-dash");
}

#[test]
fn test_binary_content_unchanged() {
    // Test that binary content is still handled as binary
    let binary_bytes = vec![0x00, 0x01, 0x02, 0x03, 0xff, 0xfe];
    
    let path = Path::new("test.bin");
    let file_info = FileInfo::from_content(path, &binary_bytes);
    
    // Binary content should not have text content
    assert!(file_info.content.is_none(), "Binary content should have no text content");
    assert_eq!(file_info.size, Some(6), "Size should be preserved");
    assert_eq!(file_info.file_name, "test.bin");
}

#[test]
fn test_ascii_content() {
    // Test that plain ASCII content works fine
    let ascii_bytes = b"Hello world!";
    
    let path = Path::new("test.txt");
    let file_info = FileInfo::from_content(path, ascii_bytes);
    
    assert!(file_info.content.is_some(), "ASCII content should not be None");
    let content = file_info.content.unwrap();
    assert_eq!(content, "Hello world!", "ASCII content should be preserved");
}