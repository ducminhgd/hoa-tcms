# Test Run

A collection of test cases executed for a specific build/environment

## Fields

1. Summary: text field. required
2. Report to: link to the user in the application, not required, default none.
3. Default tester: the PIC for the run, default: current user.
4. Project: the project that this run belongs to. Single selection field, not required.
5. Plan: the test plan that this run belongs to. Single selection field, not required.
6. Version: version of the project/product.
7. Notes: text editor.
8. Planned start: datetime picker. Not required.
9. Planned stop: datetime picker. Not required.

## List of test cases to run

User can add test cases into the test run, and they will be run by test execution(s).
The result of the test case in this test run does not effect its result in other test runs.

## List of test executions

From a test run, we can get the list of test executions that were executed for it.