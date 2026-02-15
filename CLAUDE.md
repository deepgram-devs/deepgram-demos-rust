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
- At application startup time, create a wizard that allows the user to specify any mandatory configuration values, such as an API key
  - The user should be able to skip entry, but be warned that failure to provide mandatory values may degrade application functionality

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

## User Interaction

For applications that implement a Terminal User Interface (TUI), adhere to the following guidelines:

- Keyboard shortcuts should be supported for all common operations
- Suggest ideas for and implement mouse controls such as clicking, right-clicking, scrolling, and click and drag, and drop
- Keep in mind that some terminals may not implement mouse controls, so be sure there's always a keyboard option for any mouse-driven operations
- Do not block user input for any long-running operations; the user should always be in control of the application
- The user should have the option of aborting or canceling any long-running operations running async or in separate threads
- Arrow keys should be used to navigate UI elements, such as lists of items

## Security

- Do not ever commit any kind of secret value to source control, such as API keys, SSH keys, passwords, and other sensitive information
- Ensure TLS connections are used for network operations, such as wss for WebSockets, or https for HTTP operations
  - Sometimes unencrypted non-TLS connections may be allowed for local network access, or across connections secured with a VPN

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
