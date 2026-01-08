# Demo User Feature - Implementation Plan

## 1. Objective

Implement a "Demo User" mode where a specific user (identified by email via environment variable) has **read-only access**. This user can view dashboards, stats, and lists but cannot modify data (upload tickets, delete records, update settings).

## 2. Current State

- **Authentication**: JWT-based. `AuthenticatedUser` struct in `backend/src/middleware/auth.rs` extracts the email from the token.
- **Configuration**: Loaded from environment variables in `backend/src/config.rs`.
- **Frontend**: Rust (Leptos) frontend.

## 3. Proposed Backend Changes

### 3.1 Configuration
- Add `DEMO_USER_EMAIL` to the `.env` file.
- Update `backend/src/config.rs`:
  - Add `pub demo_user_email: Option<String>` to the `Config` struct.
  - Load this value in `Config::from_env`.

### 3.2 Authentication Middleware
- Update `AuthenticatedUser` logic in `backend/src/middleware/auth.rs`:
  - Add a field `pub is_demo: bool`.
  - In `from_request_parts`, compare the extracted `claims.sub` (email) with `state.config.demo_user_email`.
  - Set `is_demo` to `true` if they match.

### 3.3 Authorization / Enforcement
- Create a new error variant in `backend/src/error.rs`:
  - `AppError::DemoUserRestriction` -> Returns generic "Action not allowed for demo user" message (e.g., 403 Forbidden).
- Enforce restrictions in sensitive route handlers (e.g., `routes/tickets.rs`, `routes/ocr.rs`):
  - Add a check at the beginning of mutation handlers:
    ```rust
    if user.is_demo {
        return Err(AppError::DemoUserRestriction);
    }
    ```
  - **Restricted Actions**:
    - Uploading tickets (OCR).
    - Deleting tickets.
    - Any other write/update operations.

## 4. Proposed Frontend Changes

### 4.1 State Management
- Update the Auth Store/Context to include the `is_demo` flag.
  - If the backend login response or user profile endpoint returns this flag, store it.
  - Alternatively, derive it strictly from the HTTP status if not explicitly passed (e.g., if we get the specific 403 `DemoUserRestriction` error). *Better approach: The backend should ideally return this flag on login/me so the UI can preemptively adapt.* (We will assume we can check the email or receive a flag).

### 4.2 Visual Cues & Restrictions
- **Global Indicator**: Show a small banner or badge "Modo Demo" in the header/sidebar.
- **Upload Button**:
  - **Option A**: Disable the "Subir Ticket" button.
  - **Option B**: Keep it enabled but show a Toast/Alert when clicked: "Como usuario demo no puedes insertar tickets. Crea un usuario para poder hacerlo." (Recommended per user request).
- **Error Handling**:
  - Catch 403 errors in the API client and show a user-friendly notification if the error code matches the demo restriction.

## 5. Implementation Steps

1.  **Backend**: Add `DEMO_USER_EMAIL` to `.env` and `Config`.
2.  **Backend**: Update `AuthenticatedUser` to populate `is_demo`.
3.  **Backend**: Add `DemoUserRestriction` error.
4.  **Backend**: Guard `POST /tickets`, `DELETE /tickets`, `POST /ocr` endpoints.
5.  **Frontend**: Add logic to display the "Demo Mode" warning when attempting restricted actions.

## 6. Open Questions
- Should the Demo User be allowed to *change* their own password? -> *Assume No.*
- Should we hide sensitive data or just prevent editing? -> *Objective says "vea pero no modifique", so viewing is fine.*

## 7. Deliverables
- [ ] Backend implementation (Config, Auth, Protections).
- [ ] Frontend implementation (UI warnings).
- [ ] `.env.example` update.
