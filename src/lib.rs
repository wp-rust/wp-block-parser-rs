use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedBlock {
    pub block_name: Option<String>,
    pub attrs: Map<String, Value>,
    pub inner_blocks: Vec<ParsedBlock>,
    pub inner_html: String,
    pub inner_content: Vec<Option<String>>,
}

impl ParsedBlock {
    pub fn new(block_name: Option<String>, attrs: Map<String, Value>) -> Self {
        Self {
            block_name,
            attrs,
            inner_blocks: Vec::new(),
            inner_html: String::new(),
            inner_content: Vec::new(),
        }
    }

    pub fn freeform(html: String) -> Self {
        let mut block = Self::new(None, Map::new());
        block.inner_html = html.clone();
        block.inner_content.push(Some(html));
        block
    }
}

#[derive(Debug, Clone, PartialEq)]
enum TokenType {
    NoMoreTokens,
    VoidBlock,
    BlockOpener,
    BlockCloser,
}

#[derive(Debug)]
struct Token {
    token_type: TokenType,
    block_name: String,
    attrs: Map<String, Value>,
    start_offset: usize,
    token_length: usize,
}

struct ParsedFrame {
    block: ParsedBlock,
    token_start: usize,
    token_length: usize,
    prev_offset: usize,
    leading_html_start: Option<usize>,
}

pub fn parse(document: &str) -> Vec<ParsedBlock> {
    let mut offset = 0;
    let mut output = Vec::new();
    let mut stack: Vec<ParsedFrame> = Vec::new();
    
    // We pass `document` slice into `next_token` along with an absolute iterator offset, 
    // but building an iterative scanner state.
    let mut scan_offset = 0;

    while proceed(document, &mut offset, &mut scan_offset, &mut output, &mut stack) {}

    output
}

fn proceed(
    document: &str,
    offset: &mut usize,
    scan_offset: &mut usize,
    output: &mut Vec<ParsedBlock>,
    stack: &mut Vec<ParsedFrame>,
) -> bool {
    let stack_depth = stack.len();
    let token = next_token(document, scan_offset);

    let leading_html_start = if token.start_offset > *offset {
        Some(*offset)
    } else {
        None
    };

    match token.token_type {
        TokenType::NoMoreTokens => {
            if stack_depth == 0 {
                add_freeform(document, offset, output, None);
                return false;
            }
            if stack_depth == 1 {
                add_block_from_stack(document, stack, output, None);
                return false;
            }
            while !stack.is_empty() {
                add_block_from_stack(document, stack, output, None);
            }
            false
        }
        TokenType::VoidBlock => {
            if stack_depth == 0 {
                if let Some(leading_start) = leading_html_start {
                    let html = &document[leading_start..token.start_offset];
                    output.push(ParsedBlock::freeform(html.to_string()));
                }
                output.push(ParsedBlock::new(Some(token.block_name), token.attrs));
                *offset = token.start_offset + token.token_length;
                return true;
            }

            add_inner_block(
                document,
                stack,
                ParsedBlock::new(Some(token.block_name), token.attrs),
                token.start_offset,
                token.token_length,
                None,
            );
            *offset = token.start_offset + token.token_length;
            true
        }
        TokenType::BlockOpener => {
            stack.push(ParsedFrame {
                block: ParsedBlock::new(Some(token.block_name), token.attrs),
                token_start: token.start_offset,
                token_length: token.token_length,
                prev_offset: token.start_offset + token.token_length,
                leading_html_start,
            });
            *offset = token.start_offset + token.token_length;
            true
        }
        TokenType::BlockCloser => {
            if stack_depth == 0 {
                add_freeform(document, offset, output, None);
                return false;
            }
            if stack_depth == 1 {
                add_block_from_stack(document, stack, output, Some(token.start_offset));
                *offset = token.start_offset + token.token_length;
                return true;
            }

            let mut stack_top = stack.pop().unwrap();
            let html = &document[stack_top.prev_offset..token.start_offset];
            stack_top.block.inner_html.push_str(html);
            stack_top.block.inner_content.push(Some(html.to_string()));
            stack_top.prev_offset = token.start_offset + token.token_length;

            add_inner_block(
                document,
                stack,
                stack_top.block,
                stack_top.token_start,
                stack_top.token_length,
                Some(token.start_offset + token.token_length),
            );
            *offset = token.start_offset + token.token_length;
            true
        }
    }
}

fn next_token(document: &str, scan_offset: &mut usize) -> Token {
    let doc = &document[*scan_offset..];
    
    // Find next <!-- wp: or <!-- /wp:
    let opener_idx = doc.find("<!-- wp:");
    let closer_idx = doc.find("<!-- /wp:");
    
    let (is_closer, match_start) = match (opener_idx, closer_idx) {
        (Some(o), Some(c)) => {
            if c < o {
                (true, c)
            } else {
                (false, o)
            }
        }
        (Some(o), None) => (false, o),
        (None, Some(c)) => (true, c),
        (None, None) => return Token {
            token_type: TokenType::NoMoreTokens,
            block_name: String::new(),
            attrs: Map::new(),
            start_offset: 0,
            token_length: 0,
        },
    };

    let start_offset = *scan_offset + match_start;
    let token_inner_start = start_offset + if is_closer { 9 } else { 8 };
    
    // Find the end of this comment
    let end_idx = document[token_inner_start..].find("-->");
    if end_idx.is_none() {
        return Token {
            token_type: TokenType::NoMoreTokens,
            block_name: String::new(),
            attrs: Map::new(),
            start_offset: 0,
            token_length: 0,
        };
    }
    
    let end_idx = token_inner_start + end_idx.unwrap();
    let token_length = (end_idx + 3) - start_offset;
    
    let mut inner = document[token_inner_start..end_idx].trim();
    
    let is_void = if inner.ends_with('/') {
        inner = inner[..inner.len()-1].trim();
        true
    } else {
        false
    };

    // The inner content is something like "core/paragraph {"align":"center"}" or just "core/paragraph"
    // Find the first space to split block name from attrs
    let space_idx = inner.find(|c: char| c.is_whitespace());
    
    let (name, attrs_str) = match space_idx {
        Some(idx) => {
            let n = inner[..idx].trim().to_string();
            let a = inner[idx..].trim();
            (n, a)
        }
        None => (inner.to_string(), ""),
    };

    // Namespace fallback
    let block_name = if name.contains('/') {
        name
    } else {
        format!("core/{}", name)
    };

    let attrs = if !attrs_str.is_empty() {
        if let Ok(Value::Object(map)) = serde_json::from_str(attrs_str) {
            map
        } else {
            Map::new()
        }
    } else {
        Map::new()
    };

    *scan_offset = start_offset + token_length;

    if is_closer {
        Token {
            token_type: TokenType::BlockCloser,
            block_name,
            attrs,
            start_offset,
            token_length,
        }
    } else if is_void {
        Token {
            token_type: TokenType::VoidBlock,
            block_name,
            attrs,
            start_offset,
            token_length,
        }
    } else {
        Token {
            token_type: TokenType::BlockOpener,
            block_name,
            attrs,
            start_offset,
            token_length,
        }
    }
}

fn add_freeform(document: &str, offset: &mut usize, output: &mut Vec<ParsedBlock>, raw_length: Option<usize>) {
    let length = raw_length.unwrap_or(document.len() - *offset);
    if length == 0 {
        return;
    }
    let html = document[*offset..*offset + length].to_string();
    output.push(ParsedBlock::freeform(html));
}

fn add_inner_block(
    document: &str,
    stack: &mut Vec<ParsedFrame>,
    block: ParsedBlock,
    token_start: usize,
    token_length: usize,
    last_offset: Option<usize>,
) {
    let parent = stack.last_mut().unwrap();
    parent.block.inner_blocks.push(block);
    
    let html = &document[parent.prev_offset..token_start];
    if !html.is_empty() {
        parent.block.inner_html.push_str(html);
        parent.block.inner_content.push(Some(html.to_string()));
    }
    
    parent.block.inner_content.push(None);
    parent.prev_offset = last_offset.unwrap_or(token_start + token_length);
}

fn add_block_from_stack(
    document: &str,
    stack: &mut Vec<ParsedFrame>,
    output: &mut Vec<ParsedBlock>,
    end_offset: Option<usize>,
) {
    let mut frame = stack.pop().unwrap();
    
    let html = if let Some(end) = end_offset {
        &document[frame.prev_offset..end]
    } else {
        &document[frame.prev_offset..]
    };

    if !html.is_empty() {
        frame.block.inner_html.push_str(html);
        frame.block.inner_content.push(Some(html.to_string()));
    }

    if let Some(leading) = frame.leading_html_start {
        let lhtml = &document[leading..frame.token_start];
        output.push(ParsedBlock::freeform(lhtml.to_string()));
    }

    output.push(frame.block);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let html = "<!-- wp:paragraph --><p>Hello</p><!-- /wp:paragraph -->";
        let blocks = parse(html);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_name, Some("core/paragraph".to_string()));
        assert_eq!(blocks[0].inner_html, "<p>Hello</p>");
    }

    #[test]
    fn test_parse_nested() {
        let html = r#"<!-- wp:group {"layout":{"type":"flex"}} -->
<div class="wp-block-group"><!-- wp:paragraph -->
<p>Inner</p>
<!-- /wp:paragraph --></div>
<!-- /wp:group -->"#;
        let blocks = parse(html);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_name, Some("core/group".to_string()));
        assert_eq!(blocks[0].inner_blocks.len(), 1);
        assert_eq!(blocks[0].inner_blocks[0].block_name, Some("core/paragraph".to_string()));
        assert_eq!(blocks[0].inner_blocks[0].inner_html, "\n<p>Inner</p>\n");
    }

    #[test]
    fn test_parse_void() {
        let html = "<!-- wp:spacer {\"height\":\"50px\"} /-->";
        let blocks = parse(html);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_name, Some("core/spacer".to_string()));
        assert_eq!(blocks[0].attrs.get("height").unwrap().as_str().unwrap(), "50px");
        assert_eq!(blocks[0].inner_html, "");
    }
}
