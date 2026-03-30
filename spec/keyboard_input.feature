Feature: Keyboard Input
  AI agents interact with TUI applications by sending keyboard input.

  Background:
    Given a session "app" is running "cat"

  Scenario: Type text
    When I run "frameterm type -s app hello"
    Then the session screen should contain "hello"

  Scenario: Send Enter key
    When I run "frameterm key -s app Enter"
    Then a newline should be sent to the session

  Scenario: Send Ctrl combo
    When I run "frameterm key -s app Ctrl+C"
    Then the interrupt signal should be sent to the session

  Scenario: Send Alt combo
    When I run "frameterm key -s app Alt+F"
    Then the Alt+F key combination should be sent

  Scenario: Send function key
    When I run "frameterm key -s app F1"
    Then the F1 key should be sent

  Scenario: Send key sequence
    When I run "frameterm key -s app 'Escape : w q Enter'"
    Then the keys Escape, :, w, q, Enter should be sent in order

  Scenario: Send key sequence with delay
    When I run "frameterm key -s app 'Tab Tab Enter' --delay 100"
    Then Tab, Tab, Enter should be sent with 100ms between each

  Scenario: Key aliases are supported
    When I run "frameterm key -s app Return"
    Then it should behave the same as sending "Enter"

  Scenario: Send Shift combo
    When I run "frameterm key -s app Shift+A"
    Then an uppercase "A" should be sent

  Scenario: Send arrow keys
    When I run "frameterm key -s app Up"
    Then the Up arrow escape sequence should be sent
    When I run "frameterm key -s app Down"
    Then the Down arrow escape sequence should be sent
    When I run "frameterm key -s app Left"
    Then the Left arrow escape sequence should be sent
    When I run "frameterm key -s app Right"
    Then the Right arrow escape sequence should be sent

  Scenario: Send navigation keys
    When I run "frameterm key -s app Home"
    Then the Home escape sequence should be sent
    When I run "frameterm key -s app End"
    Then the End escape sequence should be sent
    When I run "frameterm key -s app PageUp"
    Then the PageUp escape sequence should be sent
    When I run "frameterm key -s app PageDown"
    Then the PageDown escape sequence should be sent

  Scenario: Type to default session
    Given a session "default" is running "cat"
    When I run "frameterm type hello"
    Then the default session screen should contain "hello"

  Scenario: Invalid key name fails with suggestion
    When I run "frameterm key -s app InvalidKeyName"
    Then the command should fail with a JSON error
    And the error code should be "INVALID_KEY"
    And the suggestion should list supported key formats
    And the process exit code should be non-zero

  Scenario: Type to nonexistent session fails
    When I run "frameterm type -s nonexistent hello"
    Then the command should fail with a JSON error
    And the error code should be "SESSION_NOT_FOUND"
    And the process exit code should be non-zero
