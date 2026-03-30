Feature: Mouse Input
  AI agents can click and scroll within terminal applications.

  Background:
    Given a session "app" is running

  Scenario: Click at coordinates
    When I run "frameterm click -s app 10 5"
    Then a mouse click should be sent at row 10, column 5

  Scenario: Scroll up
    When I run "frameterm scroll -s app up"
    Then a scroll up event of 1 line should be sent

  Scenario: Scroll down multiple lines
    When I run "frameterm scroll -s app down 5"
    Then a scroll down event of 5 lines should be sent

  Scenario: Click on default session
    Given a session "default" is running
    When I run "frameterm click 10 5"
    Then the click should target the default session

  Scenario: Click with out-of-bounds coordinates
    When I run "frameterm click -s app 999 999"
    Then the command should fail with a JSON error
    And the error code should be "COORDINATES_OUT_OF_BOUNDS"
    And the message should include the terminal dimensions
    And the process exit code should be non-zero

  Scenario: Click on nonexistent session
    When I run "frameterm click -s nonexistent 10 5"
    Then the command should fail with a JSON error
    And the error code should be "SESSION_NOT_FOUND"
    And the process exit code should be non-zero

  Scenario: Scroll on nonexistent session
    When I run "frameterm scroll -s nonexistent up"
    Then the command should fail with a JSON error
    And the error code should be "SESSION_NOT_FOUND"
    And the process exit code should be non-zero
