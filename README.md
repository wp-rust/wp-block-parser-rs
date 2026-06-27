# wp-block-parser-rs

Rust port of [`@wordpress/block-serialization-default-parser`](https://github.com/WordPress/gutenberg/tree/trunk/packages/block-serialization-default-parser) from the Gutenberg project.

Parses Gutenberg block comment markup (`<!-- wp:block -->`) into a typed `ParsedBlock` tree. The fastest way to read WordPress post content in Rust — useful for content migration tools, static site generators, search indexers, and any application that processes WordPress post content.

## Installation

```toml
[dependencies]
wp-block-parser-rs = "0.1"
```

## Usage

```rust
use wp_block_parser_rs::parse;

let content = r#"
<!-- wp:heading {"level":2} -->
<h2 class="wp-block-heading">Hello World</h2>
<!-- /wp:heading -->

<!-- wp:paragraph -->
<p>Some text here.</p>
<!-- /wp:paragraph -->

<!-- wp:columns -->
<div class="wp-block-columns">
  <!-- wp:column -->
  <div class="wp-block-column"><!-- wp:paragraph --><p>Left</p><!-- /wp:paragraph --></div>
  <!-- /wp:column -->
</div>
<!-- /wp:columns -->
"#;

let blocks = parse(content);

for block in &blocks {
    println!("{:?}", block.block_name);   // Some("core/heading"), Some("core/paragraph"), ...
    println!("{:?}", block.attrs);        // JSON attributes map
    println!("{}", block.inner_content.join(""));  // raw inner HTML
}
```

## `ParsedBlock` Fields

| Field | Type | Description |
|---|---|---|
| `block_name` | `Option<String>` | Namespaced block name, e.g. `"core/paragraph"` |
| `attrs` | `Map<String, Value>` | Parsed JSON attributes from the block comment |
| `inner_blocks` | `Vec<ParsedBlock>` | Nested child blocks |
| `inner_html` | `String` | Full inner HTML (including child block markup) |
| `inner_content` | `Vec<String>` | Mixed content segments (strings + `null` placeholders) |

## Use Cases

- **Content migration** — parse existing Gutenberg posts before transforming them
- **Static site generators** — read WordPress REST API `post_content` and render to HTML
- **Search indexers** — extract plain text from block markup without regex hacks
- **Validation** — round-trip parse converted block output to verify structural correctness

## Related Crates

| Crate | Purpose |
|---|---|
| [`wp-style-engine-rs`](https://crates.io/crates/wp-style-engine-rs) | Compile block style objects to CSS |
| [`wp-wordcount-rs`](https://crates.io/crates/wp-wordcount-rs) | Count words/characters in block content |
| [`wp-escape-html-rs`](https://crates.io/crates/wp-escape-html-rs) | Sanitize HTML in block attributes |
| [`wp-token-list-rs`](https://crates.io/crates/wp-token-list-rs) | Manage block className tokens |

## License

GPL-2.0-or-later — consistent with the [Gutenberg project](https://github.com/WordPress/gutenberg).
