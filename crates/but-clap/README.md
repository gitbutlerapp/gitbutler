# But Clap

This is our documentation generation library. It uses the Clap derive 
documentation that is how we generate our help messages to create our man page
like documentation as well.

Running this command:

`cargo run --bin but-clap`

will create a new directory called `cli-docs` that is Git-ignored and populates
it with one mdx file for each command.

We use the long-about docstrings to do everything. It can take markdown (we use
the `unstable-markdown` feature in clap to parse correctly) and has some special
sections that are pulled out.

- Examples

## Testing

The `but-clap` crate includes comprehensive tests to ensure that clap-documented Rust files generate correct MDX documentation.

### Running Tests

```bash
cargo test -p but-clap
```

### Test Coverage

The tests cover various command structures:

1. **Simple commands** - Commands with basic arguments and options
2. **Commands with subcommands** - Commands that have nested subcommands
3. **Commands with default values** - Options that have default values
4. **Commands with mixed arguments** - Required and optional positional arguments
5. **Commands with hidden options** - Options marked as hidden that should not appear in docs
6. **Commands with long help text** - Commands using both short and long help descriptions
7. **Frontmatter generation** - Ensures proper YAML frontmatter for MDX files
8. **Real but command structure** - Tests against the actual `but` command to ensure compatibility

### What Tests Verify

- Proper MDX frontmatter generation (title, description)
- Correct usage string generation
- Arguments section formatting (required vs optional)
- Options section formatting (with value names, defaults, required flags)
- Subcommands section generation
- Long descriptions are properly included
- Hidden options are excluded from documentation
- Value names are correctly displayed for options that take arguments
