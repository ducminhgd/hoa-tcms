# Product requirement document

## Data object

1. [Project](/docs/PROJECT.md)
2. [Test case](/docs/TEST-CASES.md)
   1. Test case template
   2. Test Category.
3. [Test plan](/docs/TEST-PLAN.md)
4. [Test run](/docs/TEST-RUN.md)
5. [Test execution](/docs/TEST-EXECUTION.md)
6. Upload files.
7. [Identity and access management](/docs/IAM.md)
   1. Users
   2. Groups
   3. Roles
   4. Permissions
8. Integration
   1. Jira
   2. Automation test tools.

## Functional features

### General rules

1. List view:
   1. First column is the check box, to select that row.
   2. Last column is the action, often include: Duplicate, Edit, Active/Deactive, Delete. All those actions are displayed as icons.
   3. When click on the ID or the Name of the row, it will redirect the user to the update or detail page based on their permissions.
   4. Pagination:
      1. Page size: 10, 25 (default), 50, 100. Stored in cookie. Select field.
2. List page: there is the Add New button in the page. List page has filters.

### Sidebar menu tree

Top tree:

1. IAM
   1. Groups
   2. Users
   3. Roles
   4. Permissions.
2. Settings
   1. Metadata
      1. Priority
      2. Category
      3. Template
3. Test management
   1. Test Plan
   2. Test Cases
   3. Test Runs
   4. Test Executions

Bottom:
1. User profile --> user profile page with self-information update and a section to update password.
2. Logout.

### Identity and Access Management

1. Login/Logout
2. Update self-profile.
3. Update self-password.
4. There is a role `System Admin`:
   1. Has permissions create users, update users, grant permission.
   2. The first Admin user will be created through the command when init the application.
5. For an "object", there are basic permissions:
   1. Create: create the object, example: create a user. When a user has create permission, they can edit their own data.
   2. Update: update the object. Can update data from others.
   3. Read: read the detail of the object.
   4. Read List: look through the list view of the object. It is different to the Read permission, the Read permission can read all the detais of an specific object.
   5. Select: can use the selection field of the object. It is different to Read List, it only can select the ID and the Name.

### Manage metadata

1. Meta data includes:
   1. Test categories.
   2. Priorities.
2. View:
   1. List view of each type of object.
   2. Create view of each type of object.
   3. Update view of each type of object.

### Manage Project

1. There are roles to management the Project.
2. There are pages:
   1. List view of the Projects
   2. Forms: Create/Update/Detail. Detail form like Update form but all the fields are read-only.

### Manage Test cases

1. There are roles to management the Test cases.
2. There are pages:
   1. List view of the Test cases
   2. Forms: Create/Update/Detail. Detail form like Update form but all the fields are read-only.
3. Advanced (phase 2):
   1. Update or Detail form has a list of Jira issues (IDs and Links) relate to this case.
   2. Update or Detail form has a list of Jira bugs (IDs, and Links) that created from this test cases.

### Manage Test plans

1. There are roles to management the Test Plan.
2. There are pages:
   1. List view of the Test Plan
   2. Forms: Create/Update/Detail. Detail form like Update form but all the fields are read-only.
   3. Form view has a tabular to list all the Test Runs belong to the current Plan.
   4. Create or Update views has tabular to add or remove Test Runs.

### Manage Test Runs

1. There are roles to management the Test Runs.
2. There are pages:
   1. List view of the Test Runs
   2. Forms: Create/Update.
   3. Form view has a tabular to list all the Test cases belong to the current Plan.
   4. Create or Update views has tabular to add or remove Test cases.
   5. A View Detail page to view information of the test run and cannot edit anything. If user wants to edit, click edit button to redirect to Update page. There is a section as tabluar to list all the executions that run for this Test Run. There is a Statistics Section to show:
      1. Number of test run executed and split into total, in progress, success, failed.

### Manage Test Execution

1. There are roles to management the Test Executions.
2. There are pages:
   1. List view of the Test Executions
   2. Create form: name field, tester field, test run field.
      1. When choose a test run, there is a button `Import test cases` to import test cases for this test execution.
   3. Detail form contains the information of the Test Executions: name field, tester field, test run field.
      1. When choose a test run, there is a button `Import test cases` to import test cases for this test execution.
      2. In this page, user can update the result of the test cases, or go into the detail of the result.
3. Manage test result:
   1. Who have the permission of the test execution has the same permissions for its test results.
   2. User can view information of test result.
   3. User can update: result, attached files, logs of the result.
   4. Advanced (Phase 2): user can create a Jira bug ticket from this test result.

### Sharing

1. The owner can share: Project, Test Plan, Test Cases, Test Runs, Test Executions.
2. Role:
   1. Editor: can edit and share the shared object.
   2. Contributor: can edit but cannot share the object.
   3. Viewer: can view the shared object.
3. Sharing is a tab in the create page, update page, and detail page. Tabular view.

### Manage Jira integrations

1. Advanced feaure (Phase 2)
2. Use Personal Access Token.
3. Can choose Jira Projects when create or update a Project.
4. Can choose Jira Releases when create or update a Test Plan.
5. Can choose Jira Issues when create or update a Test Case.

## Non-functional feature

1. Security: avoid top 10 OWASP risks.
2. Source code MUST NOT contain any secret information.
3. Avoid memory leak and memory over-used.
4. Utilize using environment variables, YAML configurations
5. Can setup storage directory to store uploaded files.

### Technical Stack

1. PostgreSQL
2. Cache: Redis.
3. Programming language: Rust, Leptos
4. There is a Makefile for development purpose.

### Database

1. Primary key: serial or unsigned integer with auto increment.
2. N:N relationship will be another table with composite primary key.
3. Utilize JSONB data type if it is possible and necessary.