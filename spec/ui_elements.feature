Feature: UI Element Detection
  frameterm automatically detects interactive UI elements in terminal snapshots.

  Scenario: Detect buttons
    Given a session running a dialog with "[OK]" and "[Cancel]" buttons
    When I take a snapshot
    Then the elements array should contain a button with text "[OK]"
    And the elements array should contain a button with text "[Cancel]"
    And each button should have row, col, width, and confidence fields

  Scenario: Detect toggles
    Given a session running a dialog with "[x] Enable" and "[ ] Debug"
    When I take a snapshot
    Then the elements array should contain a toggle with text "[x]" and checked true
    And the elements array should contain a toggle with text "[ ]" and checked false

  Scenario: Detect input fields
    Given a session running a form with an input field at the cursor position
    When I take a snapshot
    Then the elements array should contain an input element
    And it should include the cursor row and column

  Scenario: Detect focused element
    Given a session with a focused button
    When I take a snapshot
    Then the focused button element should have "focused" set to true

  Scenario: No elements detected on plain text screen
    Given a session displaying only plain text
    When I take a snapshot
    Then the elements array should be empty
