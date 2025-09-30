use anyhow::{Result, Context};
use arboard::Clipboard;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tokio::time::{Duration, interval};

/// Clipboard content structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardContent {
    pub content_type: ContentType,
    pub data: Vec<u8>,
    pub timestamp: u64,
}

/// Type of clipboard content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentType {
    Text,
    Image,
}

impl ClipboardContent {
    /// Create a new text clipboard content
    pub fn new_text(text: String) -> Self {
        Self {
            content_type: ContentType::Text,
            data: text.into_bytes(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    
    /// Create a new image clipboard content
    pub fn new_image(data: Vec<u8>) -> Self {
        Self {
            content_type: ContentType::Image,
            data,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    
    /// Get text content if this is a text clipboard item
    pub fn text(&self) -> Option<String> {
        if let ContentType::Text = self.content_type {
            String::from_utf8(self.data.clone()).ok()
        } else {
            None
        }
    }
    
    /// Get image data if this is an image clipboard item
    pub fn image(&self) -> Option<&[u8]> {
        if let ContentType::Image = self.content_type {
            Some(&self.data)
        } else {
            None
        }
    }
}

/// Clipboard synchronization service
pub struct ClipboardSync {
    clipboard: Arc<Mutex<Clipboard>>,
    last_content: Arc<Mutex<Option<ClipboardContent>>>,
}

impl ClipboardSync {
    /// Create a new clipboard sync service
    pub fn new() -> Result<Self> {
        let clipboard = Clipboard::new()
            .context("Failed to initialize clipboard")?;
        
        Ok(Self {
            clipboard: Arc::new(Mutex::new(clipboard)),
            last_content: Arc::new(Mutex::new(None)),
        })
    }

    /// Start monitoring clipboard changes
    pub async fn start_monitoring<F>(&self, mut callback: F) -> Result<()>
    where
        F: FnMut(ClipboardContent) + Send + 'static,
    {
        println!("Starting clipboard monitoring...");
        
        let clipboard = self.clipboard.clone();
        let last_content = self.last_content.clone();
        
        // Spawn a task to monitor clipboard changes
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(500)); // Check every 500ms
            let mut previous_text: Option<String> = None;
            
            loop {
                interval.tick().await;
                
                // Try to get clipboard content
                let current_content = {
                    let mut clipboard = clipboard.lock().await;
                    clipboard.get_text().ok()
                };
                
                // Check if content has changed
                if current_content != previous_text {
                    if let Some(ref text) = current_content {
                        println!("Clipboard text changed: {}", text);
                        
                        // Check if this is different from our last sent content
                        let should_send = {
                            let last = last_content.lock().await;
                            if let Some(ref last_content) = *last {
                                if let Some(last_text) = last_content.text() {
                                    last_text != *text
                                } else {
                                    true // Last content was not text
                                }
                            } else {
                                true // No previous content
                            }
                        };
                        
                        if should_send {
                            let content = ClipboardContent::new_text(text.clone());
                            
                            // Update last content
                            {
                                let mut last = last_content.lock().await;
                                *last = Some(content.clone());
                            }
                            
                            // Call the callback with the new content
                            callback(content);
                        }
                    }
                    
                    previous_text = current_content;
                }
            }
        });
        
        Ok(())
    }

    /// Handle incoming clipboard content from network
    pub async fn handle_incoming_content(&self, content: ClipboardContent) -> Result<()> {
        println!("Received clipboard content: {:?}", content.content_type);
        
        // Update last content to prevent echo
        {
            let mut last = self.last_content.lock().await;
            *last = Some(content.clone());
        }
        
        let mut clipboard = self.clipboard.lock().await;
        
        match content.content_type {
            ContentType::Text => {
                if let Some(text) = content.text() {
                    println!("Setting clipboard text: {}", text);
                    clipboard.set_text(text)
                        .context("Failed to set clipboard text")?;
                }
            }
            ContentType::Image => {
                println!("Setting clipboard image ({} bytes)", content.data.len());
                // For images, we need to determine the format
                // This is a simplified implementation
                clipboard.set_image(arboard::ImageData {
                    width: 100,  // Placeholder values
                    height: 100, // Placeholder values
                    bytes: std::borrow::Cow::Borrowed(&content.data),
                })
                .context("Failed to set clipboard image")?;
            }
        }
        
        Ok(())
    }
}

impl Default for ClipboardSync {
    fn default() -> Self {
        Self::new().expect("Failed to create ClipboardSync")
    }
}