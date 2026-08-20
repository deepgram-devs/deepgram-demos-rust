# AGENTS.md

## Code Structure

- Be sure to use modules to segment code into logical components
- Do not write excessively long code files, use separation of concerns
- For TUI applications, design custom widgets that are reusable, and put those into separate modules
- TUI applications must reset the terminal to default state before exiting the process
- Avoid duplication of code wherever possible, don't repeat yourself

## Configuration

- If an application needs any user configuration, use a TOML file syntax in the user's home ".config" directory
  - The TOML file name should correspond to the application
  - Add appropriate inline comments describing each TOML section and option
  - Each comment should include the option's valid values, and guidance on when and how to configure it
- Users can specify environment variables that can override the TOML configuration
- Users can specify CLI options, such as --option1 --option2 to override settings in the TOML configuration file
  - CLI options have higher priority than corresponding environment variables, if both are specified
- At application startup time, warn clearly when mandatory configuration values are missing, such as a Deepgram API key or SageMaker endpoint name
  - When practical, provide an in-app entry flow for runtime overrides, such as the TTS TUI `k` key for a Deepgram API key
  - The user should be able to continue when a value is missing, but the app must explain which functionality is unavailable until it is configured
- For applications that can use multiple deployment targets, document provider-specific configuration separately and keep credentials scoped to the target that needs them
  - Example: the TTS TUI `deepgram` provider covers hosted or self-hosted Deepgram-compatible HTTP endpoints, while the `sagemaker` provider covers self-hosted Deepgram on SageMaker with AWS credentials and a SageMaker endpoint name

## User Interface Design

For Terminal User Interface (TUI) applications, keep the following items in mind regarding the user interface design:

- **Colors**: Use colors, and shades of colors, creatively to indicate what is currently selected or in focus
  - Use colors that are consistent with the brand. If you're unsure of brand colors, ask for additional guidance.
- **Themes**: Applications should support theme selection, with some common themes built-in
- **Help**: Always implement a help screen that shows the implemented keyboard shortcuts and mouse controls, if applicable.
  - The help should be displayed in a popup box, unless otherwise specified. If there's too much info to display on the screen, the user can scroll this popup dialog box to obtain a full list of help.
  - Use a question mark as the standard keyboard shortcut to display help
- **Command Palette**: Implement a feature similar to a Command Palette, which allows the user to perform a text search for, and invoke, any and all application functionality
  - The Command Palette should display the corresponding keyboard shortcut next to each item, but only if it has one
- **Progress**: Implement progress bars to show the status of long-running operations that will take more than a couple seconds
  - For long-running steps with an unknown end state, use a loading, throbber, spinner, or similar indicator to show that there's background activity happening
- **Logging**: Ensure that an application log is accessible with a keyboard shortcut, and provide detailed error messages in case a problem occurs
- **Input Validation**: When appropriate, perform user input validation and use colors or shades of color to indicate when user input is invalid, as close to real-time as possible
- **Status Bar**: Implement a status bar that provides essential information, but make sure it is subtle, not distracting, and not intrusive
- **Animations**: Use animations to show changes and activity in Terminal User Interface (TUI) applications; avoid implementing jarring user interfaces that could confuse the user
- **List Views**: Make sure that list view controls scroll correctly, as the user navigates to the top or bottom of the list.
  - Clicking on an item in a list view with the mouse should select the item

## User Interaction

For applications that implement a Terminal User Interface (TUI), adhere to the following guidelines:

- Keyboard shortcuts should be supported for all common operations
- Suggest ideas for and implement mouse controls such as clicking, right-clicking, scrolling, and click and drag, and drop
- Keep in mind that some terminals may not implement mouse controls, so be sure there's always a keyboard option for any mouse-driven operations
- Do not block user input for any long-running operations; the user should always be in control of the application
- The user should have the option of aborting or canceling any long-running operations running async or in separate threads
- Arrow keys should be used to navigate UI elements, such as lists of items
- Implement Page Up and Page Down keyboard commands for navigating list views
- Make sure that keyboard keys used to enter text into text fields do not conflict with any keyboard navigation shortcuts
- If you're creating a form for the user to fill out, make sure TAB and SHIFT+TAB are implemented to navigate between form fields, unless otherwise specified
- The mouse should scroll the Help popup screen, if it's needed

## Security

- Do not ever commit any kind of secret value to source control, such as API keys, SSH keys, passwords, and other sensitive information
- Ensure TLS connections are used for network operations, such as wss for WebSockets, or https for HTTP operations
  - Sometimes unencrypted non-TLS connections may be allowed for local network access, or across connections secured with a VPN
- Use the `internal-` prefix for git branch names that are used exclusively internally
- Do not push any git branches that start with an `internal-` prefix to GitHub public repositories
  - Pushing these to private repositories are okay

## Rust Dependency & Supply Chain Security

When modifying `Cargo.toml` or introducing external Rust crates, follow these security protocols:

### Guarding Against AI Hallucinations & Typosquatting

- Never guess a crate name or version number. Verify dependencies using authoritative registry information rather than relying on memory.
- If a crate is not part of the standard well-known ecosystem (for example, `serde`, `tokio`, `reqwest`, `clap`, or `anyhow`), verify that it exists and is valid before adding it, or request human verification.

### Mandatory Local Tool Checks

After changing `Cargo.toml` or `Cargo.lock`, run and pass the applicable project-native security checks:

- Run `cargo audit` to scan the dependency tree against the RustSec Advisory Database.
- Treat RustSec vulnerabilities as zero-tolerance: if `cargo audit` reports a vulnerability, stop the addition, roll back the change, or find an alternative patched version.
- If `deny.toml` exists in the project root, run `cargo deny check`.
- Ensure dependencies comply with the project policy for approved licenses (such as MIT/Apache-2.0), restricted sources (crates.io only), duplicate packages, and unmaintained packages.

### Supply Chain Surface Minimization

- Minimize dependency feature flags. Do not enable default features when only a subset of functionality is needed; use `default-features = false` and explicitly select the required features.
- Avoid heavy procedural macro dependencies and build dependencies that increase compile time or attack surface unless they are essential.

### Unsoundness and Unmaintained Crates

- Check whether a dependency relies extensively on `unsafe` code without documented invariants.
- If `cargo audit` or `cargo deny` reports an unmaintained dependency, explicitly flag it to the developer and propose an actively maintained alternative crate.

## Testing

- Document, recommend or implement the steps to automate testing of TUI applications
- Create an updated test plan document for manual testing operations that a human must perform, that would be too challenging or impossible to automate

## Releases

- Ensure the CHANGELOG is up-to-date with all changes since the previous release
- Ensure dependencies are updated, as long as it does not break any functionality
- Increment the application version metadata accordingly
- Ensure that the project description metadata accurately reflects the application's functionality
- Ensure the project README is updated
  - Make sure all application functionality is described in detail
  - Show a variety of example commands for different application use cases
