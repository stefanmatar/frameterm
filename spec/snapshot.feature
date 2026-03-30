Feature: Screen Snapshots
  Snapshots capture the current state of a terminal session as structured data.

  Background:
    Given a session "app" is running

  Scenario: Take a JSON snapshot
    When I run "frameterm snapshot -s app"
    Then the output should be valid JSON
    And it should contain a "size" field with "cols" and "rows"
    And it should contain a "cursor" field with "row", "col", and "visible"
    And it should contain a "text" field with the screen contents
    And it should contain a "content_hash" field
    And it should contain an "elements" array

  Scenario: Take a compact JSON snapshot
    When I run "frameterm snapshot -s app --format compact"
    Then the output should be valid JSON
    And it should not contain a "text" field

  Scenario: Take a plain text snapshot
    When I run "frameterm snapshot -s app --format text"
    Then the output should be plain text representing the screen
    And it should include a cursor position indicator

  Scenario: Snapshot of default session
    Given a session "default" is running
    When I run "frameterm snapshot"
    Then I should receive a snapshot of the default session

  Scenario: Snapshot of nonexistent session
    When I run "frameterm snapshot -s nonexistent"
    Then the command should fail with a JSON error
    And the error code should be "SESSION_NOT_FOUND"
    And the suggestion should mention "frameterm list-sessions"
    And the process exit code should be non-zero

  Scenario: Content hash changes when screen changes
    When I take a snapshot and record the content_hash
    And I run "frameterm type -s app hello"
    And I take another snapshot
    Then the content_hash should be different

  Scenario: Content hash is stable when screen is unchanged
    When I take a snapshot and record the content_hash
    And I take another snapshot
    Then the content_hash should be the same
