//! Headless Chrome backend via chromiumoxide.
//!
//! Launches a headless Chromium/Chrome instance on first navigation
//! and reuses it for subsequent commands.

use super::BrowserBackend;
use axga_shared::error::{AxgaError, AxgaResult};
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::BrowserConfig;
use chromiumoxide::cdp::browser_protocol::page::{CaptureScreenshotFormat, PrintToPdfParams};
use chromiumoxide::cdp::browser_protocol::target::CreateTargetParams;
use futures::StreamExt;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

struct HeadlessInner {
    browser: Option<chromiumoxide::Browser>,
    page: Option<chromiumoxide::Page>,
    handler_handle: Option<tokio::task::JoinHandle<()>>,
}

pub struct HeadlessBackend {
    inner: Arc<Mutex<HeadlessInner>>,
    chrome_path: Option<String>,
}

fn tool_err(msg: String) -> AxgaError {
    AxgaError::ToolError { tool: "browser".into(), message: msg }
}

impl HeadlessBackend {
    pub fn new(chrome_path: Option<String>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HeadlessInner {
                browser: None,
                page: None,
                handler_handle: None,
            })),
            chrome_path,
        }
    }

    async fn ensure_page(&self, url: &str) -> AxgaResult<chromiumoxide::Page> {
        let mut inner = self.inner.lock().await;

        if let Some(ref page) = inner.page {
            page.goto(url).await
                .map_err(|e| tool_err(format!("navigate: {e}")))?;
            return Ok(page.clone());
        }

        let mut config_builder = BrowserConfig::builder()
            .with_head()
            .no_sandbox();
        if let Some(ref path) = self.chrome_path {
            config_builder = config_builder.chrome_executable(path);
        }
        let config = config_builder
            .build()
            .map_err(|e| tool_err(format!("config: {e}")))?;

        let (browser, mut handler) = chromiumoxide::Browser::launch(config)
            .await
            .map_err(|e| tool_err(format!("launch: {e}")))?;

        let handle = tokio::spawn(async move {
            while let Some(Ok(())) = handler.next().await {}
        });

        let page = browser
            .new_page(CreateTargetParams {
                url: url.to_string(),
                width: Some(1920i64),
                height: Some(1080i64),
                browser_context_id: None,
                enable_begin_frame_control: None,
                new_window: None,
                background: None,
                for_tab: None,
            })
            .await
            .map_err(|e| tool_err(format!("new_page: {e}")))?;

        inner.browser = Some(browser);
        inner.page = Some(page.clone());
        inner.handler_handle = Some(handle);

        Ok(page)
    }
}

#[async_trait::async_trait]
impl BrowserBackend for HeadlessBackend {
    async fn navigate(&self, url: &str) -> AxgaResult<()> {
        self.ensure_page(url).await?;
        Ok(())
    }

    async fn snapshot(&self) -> AxgaResult<String> {
        let inner = self.inner.lock().await;
        match inner.page {
            Some(ref page) => page.content().await
                .map_err(|e| tool_err(format!("snapshot: {e}"))),
            None => Err(tool_err("not started".into())),
        }
    }

    async fn click(&self, selector: &str) -> AxgaResult<()> {
        let inner = self.inner.lock().await;
        match inner.page {
            Some(ref page) => {
                page.find_element(selector).await
                    .map_err(|e| tool_err(format!("find: {e}")))?
                    .click().await
                    .map_err(|e| tool_err(format!("click: {e}")))?;
                Ok(())
            }
            None => Err(tool_err("not started".into())),
        }
    }

    async fn fill(&self, selector: &str, text: &str) -> AxgaResult<()> {
        let inner = self.inner.lock().await;
        match inner.page {
            Some(ref page) => {
                page.find_element(selector).await
                    .map_err(|e| tool_err(format!("find: {e}")))?
                    .type_str(text).await
                    .map_err(|e| tool_err(format!("fill: {e}")))?;
                Ok(())
            }
            None => Err(tool_err("not started".into())),
        }
    }

    async fn execute_js(&self, js: &str) -> AxgaResult<Value> {
        let inner = self.inner.lock().await;
        match inner.page {
            Some(ref page) => {
                let result = page.evaluate_expression(js).await
                    .map_err(|e| tool_err(format!("js: {e}")))?;
                result.value()
                    .cloned()
                    .ok_or_else(|| tool_err("js returned null or undefined".into()))
            }
            None => Err(tool_err("not started".into())),
        }
    }

    async fn screenshot(&self) -> AxgaResult<Vec<u8>> {
        let inner = self.inner.lock().await;
        match inner.page {
            Some(ref page) => {
                let params = ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .full_page(true)
                    .build();
                page.screenshot(params).await
                    .map_err(|e| tool_err(format!("screenshot: {e}")))
            }
            None => Err(tool_err("not started".into())),
        }
    }

    async fn pdf(&self) -> AxgaResult<Vec<u8>> {
        let inner = self.inner.lock().await;
        match inner.page {
            Some(ref page) => {
                page.pdf(PrintToPdfParams::default()).await
                    .map_err(|e| tool_err(format!("pdf: {e}")))
            }
            None => Err(tool_err("not started".into())),
        }
    }
}
