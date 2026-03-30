Feature: Wait For Text
  Agents can block until specific text or patterns appear on screen.

  Background:
    Given a session "app" is running

  Scenario: Wait for literal text
    When the application will eventually display "Ready"
    And I run "frameterm wait-for -s app Ready"
    Then the command should return once "Ready" appears on screen

  Scenario: Wait for regex pattern
    When the application will eventually display "Error: code 42"
    And I run "frameterm wait-for -s app 'Error: code \d+' --regex"
    Then the command should return once the pattern matches

  Scenario: Wait for text with custom timeout
    When I run "frameterm wait-for -s app 'Never appears' --timeout 2000"
    Then the command should fail with a JSON error after 2000ms
    And the error code should be "WAIT_TIMEOUT"
    And the process exit code should be non-zero

  Scenario: Default timeout for wait-for
    When I run "frameterm wait-for -s app 'Never appears'"
    Then the command should fail with a JSON error after 30 seconds
    And the error code should be "WAIT_TIMEOUT"
    And the process exit code should be non-zero

  Scenario: Wait for text on default session
    Given a session "default" is running
    When the application will eventually display "Loaded"
    And I run "frameterm wait-for Loaded"
    Then it should wait on the default session

  Scenario: Text already present on screen
    Given the screen already contains "Welcome"
    When I run "frameterm wait-for -s app Welcome"
    Then the command should return immediately

  Scenario: Wait for text on nonexistent session
    When I run "frameterm wait-for -s nonexistent Ready"
    Then the command should fail with a JSON error
    And the error code should be "SESSION_NOT_FOUND"
    And the process exit code should be non-zero
