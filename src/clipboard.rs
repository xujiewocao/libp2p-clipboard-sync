use anyhow::{Result, Context};
use arboard::Clipboard;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
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
    // Add width and height for image content
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub from_network: bool,
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
            from_network: false,
            width: None,
            height: None,
        }
    }
    
    /// Create a new image clipboard content
    pub fn new_image(data: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            content_type: ContentType::Image,
            data,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            width: Some(width),
            height: Some(height),
            from_network: false,
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
#[derive(Clone)]
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
            let mut previous_image_hash: Option<u64> = None; // Track image changes by hash
            
            loop {
                interval.tick().await;
                
                // Try to get clipboard content (both text and image)
                let current_text = {
                    let mut clipboard = clipboard.lock().await;
                    clipboard.get_text().ok()
                };
                
                let current_image_data = {
                    let mut clipboard = clipboard.lock().await;
                    clipboard.get_image().ok().map(|img_data| {
                        // Convert image data to bytes and get dimensions
                        (img_data.bytes.to_vec(), img_data.width as u32, img_data.height as u32)
                    })
                };
                
                // Check if text content has changed
                if current_text != previous_text {
                    if let Some(ref text) = current_text {
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
                            let mut content = ClipboardContent::new_text(text.clone());
                            // Mark as coming from network
                            content.from_network = true;
                            // Update last content
                            {
                                let mut last = last_content.lock().await;
                                *last = Some(content.clone());
                            }
                            
                            // Call the callback with the new content
                            callback(content);
                        }
                    }
                    
                    previous_text = current_text;
                    // Reset image hash since we're dealing with text now
                    previous_image_hash = None;
                }
                // Check if image content has changed
                else if let Some((image_data, width, height)) = current_image_data {
                    // Calculate hash of image data to detect changes
                    let image_hash = {
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::Hasher;
                        let mut hasher = DefaultHasher::new();
                        hasher.write(&image_data);
                        hasher.finish()
                    };
                    
                    if Some(image_hash) != previous_image_hash {
                        println!("Clipboard image changed ({} bytes, {}x{})", image_data.len(), width, height);
                        
                        let content = ClipboardContent::new_image(image_data.clone(), width, height);
                        
                        // Update last content
                        {
                            let mut last = last_content.lock().await;
                            *last = Some(content.clone());
                        }
                        
                        // Call the callback with the new content
                        callback(content);
                        
                        previous_image_hash = Some(image_hash);
                    }
                } else {
                    // No image data available, reset image hash
                    previous_image_hash = None;
                }
            }
        });
        
        Ok(())
    }

    /// Handle incoming clipboard content from network
    pub async fn handle_incoming_content(&self, content: ClipboardContent) -> Result<()> {
        println!("Received clipboard content: {:?} ({}x{})", content.content_type, 
                 content.width.unwrap_or(0), content.height.unwrap_or(0));
        
        // Update last content to prevent echo
        {
            let mut last = self.last_content.lock().await;
            *last = Some(content.clone());
        }
        
        let result = {
            let mut clipboard = self.clipboard.lock().await;
            
            match content.content_type {
                ContentType::Text => {
                    if let Some(text) = content.text() {
                        println!("Setting clipboard text: {}", text);
                        clipboard.set_text(text)
                            .context("Failed to set clipboard text")
                    } else {
                        Ok(())
                    }
                }
                ContentType::Image => {
                    if let Some(image_data) = content.image() {
                        println!("Setting clipboard image ({} bytes, {}x{})", 
                                 image_data.len(), 
                                 content.width.unwrap_or(0), 
                                 content.height.unwrap_or(0));
                        
                        // Create proper ImageData from the received bytes with correct dimensions
                        clipboard.set_image(arboard::ImageData {
                            width: content.width.unwrap_or(100) as usize,  // Use received width or default
                            height: content.height.unwrap_or(100) as usize, // Use received height or default
                            bytes: std::borrow::Cow::Borrowed(image_data),
                        })
                        .context("Failed to set clipboard image")
                    } else {
                        Ok(())
                    }
                }
            }
        };
        
        result
    }
}

impl Default for ClipboardSync {
    fn default() -> Self {
        Self::new().expect("Failed to create ClipboardSync")
    }
}