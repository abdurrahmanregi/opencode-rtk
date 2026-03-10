use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, Strategy},
    Context,
};
use anyhow::Result;

pub struct PrismaModule {
    strategy: ErrorOnly,
}

impl PrismaModule {
    pub fn new() -> Self {
        Self {
            strategy: ErrorOnly,
        }
    }
}

impl Default for PrismaModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for PrismaModule {
    fn name(&self) -> &str {
        "prisma"
    }

    fn strategy(&self) -> &str {
        self.strategy.name()
    }

    fn compress(&self, output: &str, _context: &Context) -> Result<String> {
        self.strategy.compress(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(command: &str) -> Context {
        Context {
            cwd: "/tmp".to_string(),
            exit_code: 0,
            tool: "bash".to_string(),
            session_id: None,
            command: Some(command.to_string()),
        }
    }

    #[test]
    fn test_empty() {
        let module = PrismaModule::new();
        let result = module.compress("", &make_context("prisma")).unwrap();
        assert_eq!(result, "(empty)");
    }

    #[test]
    fn test_prisma_migrate_success() {
        let module = PrismaModule::new();
        let input = r#"Environment variables loaded from .env
Prisma schema loaded from prisma/schema.prisma

Datasource "db": PostgreSQL database "mydb", schema "public" at "localhost:5432"

14 migrations found in prisma/migrations

No pending migrations to apply.
"#;
        let result = module
            .compress(input, &make_context("prisma migrate dev"))
            .unwrap();

        // No errors
        assert_eq!(result, "(no errors)");
    }

    #[test]
    fn test_prisma_migrate_error() {
        let module = PrismaModule::new();
        let input = r#"Environment variables loaded from .env
Prisma schema loaded from prisma/schema.prisma

Datasource "db": PostgreSQL database "mydb", schema "public" at "localhost:5432"

Error: P1001

Can't reach database server at `localhost:5432`

Please make sure your database server is running at `localhost:5432`.
"#;
        let result = module
            .compress(input, &make_context("prisma migrate dev"))
            .unwrap();

        assert!(result.contains("Error") || result.contains("Can't reach"));
    }

    #[test]
    fn test_prisma_generate_success() {
        let module = PrismaModule::new();
        let input = r#"Environment variables loaded from .env
Prisma schema loaded from prisma/schema.prisma

✔ Generated Prisma Client (4.15.0 | library) to ./node_modules/@prisma/client in 234ms

You can now start using Prisma Client in your code. Reference: https://pris.ly/d/client

import { PrismaClient } from '@prisma/client'
const prisma = new PrismaClient()
"#;
        let result = module
            .compress(input, &make_context("prisma generate"))
            .unwrap();

        assert_eq!(result, "(no errors)");
    }

    #[test]
    fn test_prisma_generate_error() {
        let module = PrismaModule::new();
        let input = r#"Environment variables loaded from .env
Prisma schema loaded from prisma/schema.prisma

Error: Prisma schema validation failed
Error code: P1012

error: Error validating model "User": The field "id" must be required.
  -->  schema.prisma:5
   | 
 4 |   model User {
 5 |     id Int
   | 
"#;
        let result = module
            .compress(input, &make_context("prisma generate"))
            .unwrap();

        assert!(result.contains("Error"));
    }

    #[test]
    fn test_prisma_studio() {
        let module = PrismaModule::new();
        let input = r#"Environment variables loaded from .env
Prisma schema loaded from prisma/schema.prisma

Prisma Studio is up on http://localhost:5555
"#;
        let result = module
            .compress(input, &make_context("prisma studio"))
            .unwrap();

        assert_eq!(result, "(no errors)");
    }

    #[test]
    fn test_prisma_db_push_error() {
        let module = PrismaModule::new();
        let input = r#"Environment variables loaded from .env
Prisma schema loaded from prisma/schema.prisma

Datasource "db": PostgreSQL database "mydb", schema "public" at "localhost:5432"

Error: P2002

Unique constraint failed on the fields: (`email`)
"#;
        let result = module
            .compress(input, &make_context("prisma db push"))
            .unwrap();

        assert!(result.contains("Error") || result.contains("failed"));
    }

    #[test]
    fn test_prisma_seed_success() {
        let module = PrismaModule::new();
        let input = r#"Environment variables loaded from .env
Running seed command `ts-node prisma/seed.ts` ...

🌱  The seed command has been executed.
"#;
        let result = module
            .compress(input, &make_context("prisma db seed"))
            .unwrap();

        assert_eq!(result, "(no errors)");
    }

    #[test]
    fn test_prisma_seed_error() {
        let module = PrismaModule::new();
        let input = r#"Environment variables loaded from .env
Running seed command `ts-node prisma/seed.ts` ...

Error: Command failed with exit code 1: ts-node prisma/seed.ts
An error occurred during seed execution
"#;
        let result = module
            .compress(input, &make_context("prisma db seed"))
            .unwrap();

        assert!(result.contains("Error") || result.contains("failed"));
    }
}
