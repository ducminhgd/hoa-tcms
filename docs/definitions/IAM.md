# Identity and Access Management

## User Group

### Fields

1. Name: text field, required
2. Description: text editor.
3. Status: selection field, ACTIVE or INACTIVE, default ACTIVE.
4. Roles: tabular, a list of roles that give to this groups.

## User

### Fields

1. Username: text field, required, unique.
2. Email: text field, required, unique.
3. Password: text field, will be stored as a pbkdf2 string with format `pbkdf2$<algorithm>$<salt>$<iteration_num>$<hashed_string>`
4. Fullname: text field, required, default same as username.
5. Status: selection field, ACTIVE or INACTIVE, default ACTIVE.
6. Roles: tabular, a list of roles that grant to this user.

### Rules

1. A user can login with [(username OR email) AND password].
2. A user can be belong in multiple groups.

## Permission

### Fields

1. Name: text field, required
2. Code: text field, unique, required.

## Role

A set of permissions.

### Fields

1. Name: text field, required.
2. Permissions: Tabular, list of permissions for the role.

## Rules

1. If a user has a role, they have all the permissions of that role.
2. If a group has a role, all its users have all the permissions of the role.