use raii::Raii;
use {ffi, Handle, Return};
use super::types::OdbcType;

/// Indicates that a type can be retrieved using `Cursor::get_data`
pub unsafe trait Output<'a>: Sized {
    fn get_data(
        stmt: &mut Raii<ffi::Stmt>,
        col_or_param_num: u16,
        buffer: &'a mut Vec<u8>,
    ) -> Return<Option<Self>>;
}

unsafe impl<'a, T> Output<'a> for T
where
    T: OdbcType<'a>,
{
    fn get_data(
        stmt: &mut Raii<ffi::Stmt>,
        col_or_param_num: u16,
        buffer: &'a mut Vec<u8>,
    ) -> Return<Option<Self>> {
        stmt.get_data(col_or_param_num, buffer)
    }
}

impl Raii<ffi::Stmt> {
    fn get_data<'a, T>(
        &mut self,
        col_or_param_num: u16,
        buffer: &'a mut Vec<u8>
    ) -> Return<Option<T>>
    where
        T: OdbcType<'a>,
    {
        self.get_partial_data(col_or_param_num, buffer, 0)
    }

    fn get_partial_data<'a, T>(
        &mut self,
        col_or_param_num: u16,
        buffer: &'a mut Vec<u8>,
        start_pos: usize
    ) -> Return<Option<T>>
    where
        T: OdbcType<'a>,
    {
        if buffer.len() - start_pos == 0 {
            panic!("buffer length may not be zero");
        }
        if buffer.len() - start_pos > ffi::SQLLEN::max_value() as usize {
            panic!("buffer is larger than {} bytes", ffi::SQLLEN::max_value());
        }
        let mut indicator: ffi::SQLLEN = 0;
        // Get buffer length...
        let result = unsafe { ffi::SQLGetData(
                self.handle(),
                col_or_param_num,
                T::c_data_type(),
                buffer.as_mut_ptr().offset(start_pos as isize) as ffi::SQLPOINTER,
                (buffer.len() - start_pos) as ffi::SQLLEN,
                &mut indicator as *mut ffi::SQLLEN,
            ) };
        match result {
            ffi::SQL_SUCCESS => {
                if indicator == ffi::SQL_NULL_DATA {
                    Return::Success(None)
                } else {
                    let slice = &buffer[..(start_pos + indicator as usize)];
                    Return::Success(Some(T::convert(slice)))
                }
            }
            ffi::SQL_SUCCESS_WITH_INFO => {
                let initial_len = buffer.len();
                if indicator == ffi::SQL_NO_TOTAL {
                    buffer.resize(initial_len * 2, 0);
                    return self.get_partial_data(col_or_param_num, buffer, initial_len - 1);
                } else {
                    // Check if string has been truncated.
                    if indicator >= initial_len as ffi::SQLLEN {
                        buffer.resize(indicator as usize + 1, 0);
                        return self.get_partial_data(col_or_param_num, buffer, initial_len - 1);
                    } else {
                        let slice = &buffer[..(start_pos + indicator as usize)];
                        // No truncation. Warning may be due to some other issue.
                        Return::SuccessWithInfo(Some(T::convert(slice)))
                    }
                }
            }
            ffi::SQL_ERROR => Return::Error,
            ffi::SQL_NO_DATA => panic!("SQLGetData has already returned the colmun data"),
            r => panic!("unexpected return value from SQLGetData: {:?}", r),
        }
    }
}
