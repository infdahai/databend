//  Copyright 2021 Datafuse Labs.
//
//  Licensed under the Apache License, Version 2.0 (the "License");
//  you may not use this file except in compliance with the License.
//  You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
//

use common_base::tokio;
use common_datablocks::assert_blocks_sorted_eq;
use common_datablocks::DataBlock;
use common_datavalues::prelude::*;
use common_exception::Result;
use common_meta_types::TableInfo;
use common_meta_types::TableMeta;
use common_planners::*;
use futures::TryStreamExt;

use crate::catalogs1::ToReadDataSourcePlan;
use crate::storages::memory::MemoryTable;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_memorytable() -> Result<()> {
    let ctx = crate::tests::try_create_context()?;
    let schema = DataSchemaRefExt::create(vec![
        DataField::new("a", DataType::UInt64, false),
        DataField::new("b", DataType::UInt64, false),
    ]);
    let table = MemoryTable::try_create(crate::tests::try_create_storage_context()?, TableInfo {
        desc: "'default'.'a'".into(),
        name: "a".into(),
        ident: Default::default(),
        meta: TableMeta {
            schema: schema.clone(),
            engine: "Memory".to_string(),
            options: TableOptions::default(),
        },
    })?;

    // append data.
    {
        let block = DataBlock::create_by_array(schema.clone(), vec![
            Series::new(vec![1u64, 2]),
            Series::new(vec![11u64, 22]),
        ]);
        let block2 = DataBlock::create_by_array(schema.clone(), vec![
            Series::new(vec![4u64, 3]),
            Series::new(vec![33u64, 33]),
        ]);
        let blocks = vec![Ok(block), Ok(block2)];

        let input_stream = futures::stream::iter::<Vec<Result<DataBlock>>>(blocks.clone());
        table
            .append_data(ctx.clone(), Box::pin(input_stream))
            .await
            .unwrap();
    }

    // read.
    {
        let source_plan = table.read_plan(ctx.clone(), None).await?;
        ctx.try_set_partitions(source_plan.parts.clone())?;
        assert_eq!(table.engine(), "Memory");

        let stream = table.read(ctx.clone(), &source_plan).await?;
        let result = stream.try_collect::<Vec<_>>().await?;
        assert_blocks_sorted_eq(
            vec![
                "+---+----+",
                "| a | b  |",
                "+---+----+",
                "| 1 | 11 |",
                "| 2 | 22 |",
                "| 3 | 33 |",
                "| 4 | 33 |",
                "+---+----+",
            ],
            &result,
        );
    }

    // truncate.
    {
        let truncate_plan = TruncateTablePlan {
            db: "default".to_string(),
            table: "a".to_string(),
        };
        table.truncate(ctx.clone(), truncate_plan).await?;

        let source_plan = table.read_plan(ctx.clone(), None).await?;
        let stream = table.read(ctx, &source_plan).await?;
        let result = stream.try_collect::<Vec<_>>().await?;
        assert_blocks_sorted_eq(vec!["++", "++"], &result);
    }

    Ok(())
}
