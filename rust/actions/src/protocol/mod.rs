pub const ACTION_PROTO: &str = include_str!("action.proto");

pub mod grpc {
	tonic::include_proto!("actions");
}
