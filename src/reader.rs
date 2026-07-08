//! Parquet reader: reads a downloaded parquet file, converts WKB geometry to
//! WKT strings, and returns an Arrow StructArray suitable for Hayashi DataFrame.

use arrow::array::{
    Array, ArrayRef, BinaryArray, BooleanArray, Float64Array, Int64Array,
    NullArray, RecordBatch, RecordBatchOptions, RecordBatchReader, StringArray,
    StructArray,
};
use arrow::datatypes::{DataType, Field};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;
use std::sync::Arc;

use crate::wkb::wkb_to_wkt;

/// Read a parquet file and return a StructArray that Hayashi interprets as a
/// DataFrame. Geometry columns (WKB binary) are converted to WKT strings.
pub fn read_parquet_to_struct(
    path: &std::path::Path,
    filter_col: Option<&str>,
    filter_val: Option<&str>,
) -> Result<StructArray, String> {
    let file = File::open(path)
        .map_err(|e| format!("cannot open {}: {e}", path.display()))?;

    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("cannot read parquet: {e}"))?;
    let reader = builder.build().map_err(|e| format!("parquet reader: {e}"))?;

    let schema = reader.schema();
    let fields: Vec<Arc<Field>> = schema.fields().iter().cloned().collect();

    // Collect all batches, applying filter if specified
    let mut filtered_batches: Vec<RecordBatch> = Vec::new();
    for batch in reader {
        let batch = batch.map_err(|e| format!("parquet read error: {e}"))?;

        if let (Some(col), Some(val)) = (filter_col, filter_val) {
            let filtered = filter_batch(&batch, col, val)?;
            if filtered.num_rows() > 0 {
                filtered_batches.push(filtered);
            }
        } else {
            filtered_batches.push(batch);
        }
    }

    if filtered_batches.is_empty() {
        // Return empty struct with the schema
        let empty_arrays: Vec<ArrayRef> = fields
            .iter()
            .map(|f: &Arc<Field>| make_empty_array(f.data_type()))
            .collect();
        return Ok(StructArray::new(fields.into(), empty_arrays, None));
    }

    // Concatenate batches
    let combined = if filtered_batches.len() == 1 {
        filtered_batches.into_iter().next().unwrap()
    } else {
        arrow::compute::concat_batches(&schema, &filtered_batches)
            .map_err(|e| format!("concat error: {e}"))?
    };

    // Convert columns: WKB binary → WKT string, Float64 → keep, others → keep
    let mut out_fields: Vec<Arc<Field>> = Vec::new();
    let mut out_arrays: Vec<ArrayRef> = Vec::new();

    for (i, field) in combined.schema().fields().iter().enumerate() {
        let col = combined.column(i);

        if field.name() == "geometry" && *field.data_type() == DataType::Binary {
            // Convert WKB → WKT
            let binary = col
                .as_any()
                .downcast_ref::<BinaryArray>()
                .ok_or("geometry column is not Binary")?;

            let wkt_strings: Vec<Option<String>> = (0..binary.len())
                .map(|i| {
                    if binary.is_null(i) {
                        None
                    } else {
                        let wkb = binary.value(i);
                        wkb_to_wkt(wkb).ok()
                    }
                })
                .collect();

            let wkt_field = Arc::new(Field::new("geometry", DataType::Utf8, true));
            let wkt_array = Arc::new(StringArray::from(wkt_strings)) as ArrayRef;
            out_fields.push(wkt_field);
            out_arrays.push(wkt_array);
        } else {
            // Keep column as-is
            out_fields.push(field.clone());
            out_arrays.push(col.clone());
        }
    }

    Ok(StructArray::new(out_fields.into(), out_arrays, None))
}

/// Filter a batch by a column value (string comparison).
fn filter_batch(
    batch: &RecordBatch,
    col_name: &str,
    value: &str,
) -> Result<RecordBatch, String> {
    let schema = batch.schema();
    let field_idx = schema
        .fields()
        .iter()
        .position(|f| f.name() == col_name)
        .ok_or_else(|| format!("filter column '{col_name}' not found"))?;

    let col = batch.column(field_idx);
    let dt = col.data_type();

    // Build a filter mask
    let mask: Vec<bool> = match dt {
        DataType::Utf8 => {
            let arr = col.as_any().downcast_ref::<StringArray>().unwrap();
            (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        false
                    } else {
                        arr.value(i) == value
                    }
                })
                .collect()
        }
        DataType::Float64 => {
            let arr = col.as_any().downcast_ref::<Float64Array>().unwrap();
            // Try parsing value as f64
            let val_f: f64 = value.parse().unwrap_or(f64::NAN);
            (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        false
                    } else {
                        (arr.value(i) - val_f).abs() < 1e-10
                    }
                })
                .collect()
        }
        DataType::Int64 => {
            let arr = col.as_any().downcast_ref::<Int64Array>().unwrap();
            let val_i: i64 = value.parse().unwrap_or(i64::MIN);
            (0..arr.len())
                .map(|i| {
                    if arr.is_null(i) {
                        false
                    } else {
                        arr.value(i) == val_i
                    }
                })
                .collect()
        }
        _ => {
            return Err(format!(
                "cannot filter column '{col_name}' of type {dt}"
            ))
        }
    };

    // Apply filter to each column
    let filter_array = arrow::array::BooleanArray::from(mask);
    let filtered_cols: Vec<ArrayRef> = batch
        .columns()
        .iter()
        .map(|c| arrow::compute::filter(c, &filter_array).unwrap_or_else(|_| c.clone()))
        .collect();

    let options = RecordBatchOptions::new().with_row_count(None);
    RecordBatch::try_new_with_options(
        schema,
        filtered_cols,
        &options,
    )
    .map_err(|e| format!("filter error: {e}"))
}

fn make_empty_array(dt: &DataType) -> ArrayRef {
    match dt {
        DataType::Utf8 => Arc::new(StringArray::new_null(0)),
        DataType::Float64 => Arc::new(Float64Array::new_null(0)),
        DataType::Int64 => Arc::new(Int64Array::new_null(0)),
        DataType::Boolean => Arc::new(BooleanArray::new_null(0)),
        DataType::Binary => Arc::new(BinaryArray::new_null(0)),
        _ => Arc::new(NullArray::new(0)),
    }
}
